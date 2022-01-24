use llama::{
    transforms::{
        instcombine::LLVMAddInstructionCombiningPass,
        scalar::{
            LLVMAddCFGSimplificationPass, LLVMAddGVNPass, LLVMAddIndVarSimplifyPass,
            LLVMAddLoopUnrollPass, LLVMAddReassociatePass,
        },
    },
    Builder, Const, Context, ExecutionEngine, FuncPassManager, FuncType, Icmp, Module, PassManager,
    Type, Value,
};

use crate::{
    jit::CodeGen,
    util::{self, GenerateCode, StaticGetType},
    wrappers::ModifyInPlace,
};

pub struct InstantiateSliceOptions {
    /// Whether it should log some information, such as the IR before opt
    /// and the IR after opt, as well as the function pointer.
    pub log: bool,
    /// A specific count that we should optimize for.
    /// The passed in elements when calling *must* have the same number of elements as `count`
    pub count: Option<usize>,
}

#[derive(Debug)]
pub enum InstantiateError {
    /// An error from LLVM/llama
    Llama(llama::Error),
    /// The function we tried getting out was a nullptr
    EngineFunctionNull,
}
impl From<llama::Error> for InstantiateError {
    fn from(err: llama::Error) -> Self {
        InstantiateError::Llama(err)
    }
}

type ModifySliceInPlaceFn<Target> = unsafe extern "C" fn(*mut Target, usize);
impl<Target: 'static + StaticGetType, Actions: 'static + GenerateCode>
    ModifyInPlace<Target, Actions>
{
    /// Instantiate a function over a slice with some statically known data
    pub fn instantiate_slice(
        &self,
        opt: InstantiateSliceOptions,
    ) -> Result<InstantiatedSliceModifyInPlace<Target>, InstantiateError> {
        let cg = {
            let context = Context::new()?;
            let module = Module::new(&context, "instantiate_slice")?;
            let build = Builder::new(&context)?;
            let engine = ExecutionEngine::new_jit(module, 3)?;
            CodeGen {
                context,
                build,
                engine,
            }
        };

        // TODO: Use platform specific usize
        let void_ty = Type::void(&cg.context)?;
        let target_pointer_ty = Target::static_get_pointer_type(&cg.context, None)?;
        let usize_ty = Type::i64(&cg.context)?;
        // The type of the function
        let instantiate_ty = FuncType::new(void_ty, [target_pointer_ty, usize_ty])?;

        let actions = &self.actions;
        let func = cg.engine.module().declare_function(
            &cg.build,
            "instantiate_slice",
            instantiate_ty,
            |f| {
                let params = f.params();
                let target_data = params[0];
                // Use the given count if needed
                let length = if let Some(count) = opt.count {
                    Value::from(Const::int(usize_ty, count as i64)?)
                } else {
                    params[1]
                };

                // TODO: Check when building the function if the provided count would cause the mul
                // to overflow. If there was no provided count then check at runtime if the mul
                // would overflow.

                let zero_v = Const::int(usize_ty, 0)?;
                let one_v = Const::int(usize_ty, 1)?;
                cg.build.for_loop(
                    zero_v,
                    |index| {
                        cg.build
                            .icmp(Icmp::LLVMIntULT, index, length, "should_continue")
                    },
                    |index| cg.build.add(index, one_v, "next_index"),
                    |index| {
                        let data_ptr = cg.build.gep(target_data, &[*index], "element_ptr")?;
                        // let offset = cg.build.mul(index, target_size_v, "offset_mul")?;
                        // let data_ptr = cg.build.add(target_data, offset, "data_add")?;
                        actions.generate_code(&cg, Value::from(data_ptr))?;
                        Ok(*index)
                    },
                )?;

                cg.build.ret_void()
            },
        )?;

        if opt.log {
            println!("\n\nPre-Optimized Module: \n{}", cg.engine.module());
        }

        // Verify the function, just to make sure we don't miss some problems with it
        func.verify()?;

        // Instantiate passes for optimizing
        let fp = FuncPassManager::new(cg.engine.module())?;
        // TODO: I feel like this should be unsafe, because it literally just runs these passes..
        // which are just unsafe function pointers
        // TODO: We can probably add more, and/or let the caller specify expected ones that might
        // be useful
        fp.add(&[
            LLVMAddInstructionCombiningPass,
            LLVMAddReassociatePass,
            LLVMAddGVNPass,
            LLVMAddCFGSimplificationPass,
            LLVMAddIndVarSimplifyPass,
            LLVMAddLoopUnrollPass,
        ]);
        // Apply the transformations
        fp.run(&func);

        if opt.log {
            // Log the IR after optimizations
            println!("\n\nPost-Optimized Module: \n{}", cg.engine.module());
        }

        // TODO: Can we have better checks on this to ensure that there wasn't any weirdness?
        // Get the function pointer that it created, we can't immediately cast it to
        // `ModifySliceInPlaceFn` because function pointer types can't be null, so we use option
        // around it, which is suggested by UCG
        let func: Option<ModifySliceInPlaceFn<Target>> =
            unsafe { cg.engine.function("instantiate_slice") }?;
        // If it is null, then that's some unknown error
        let func = func.ok_or(InstantiateError::EngineFunctionNull)?;

        let inst = unsafe {
            // Safety:
            // The given CG was used to create the function
            // The options were used in the creation/optimization of the code
            InstantiatedSliceModifyInPlace::new(cg, opt, func)
        };

        Ok(inst)
    }
}

pub struct InstantiatedSliceModifyInPlace<'a, Target> {
    // TODO: We may not need to keep all of this alive,
    // TODO: there may be a way to divorce the
    // ownership (aka make the function live for 'static, like mem::leak)
    #[allow(dead_code)]
    /// This must be kept alive for the function to be valid
    cg: CodeGen<'a>,
    // TODO: In the future, we might have options which we don't need to keep around for verifying
    // the safety of the call.
    /// The options that were given for creating the function.
    options: InstantiateSliceOptions,
    /// (data, length)
    /// SAFETY: This must not hold a pointer/reference to the value behind it after the completion
    /// of the function.
    /// SAFETY: The pointer must point to a valid allocation where we may safely index into each
    /// entry up to length.
    func: ModifySliceInPlaceFn<Target>,
}
impl<'a, Target> InstantiatedSliceModifyInPlace<'a, Target> {
    /// This is unsafe because construction of this structure has to satisfy contrainsts that are
    /// hard or impossible to verify.
    /// # Safety
    /// `cg` *must* be the [`CodeGen`] instances used to create (and thus owns) `func`
    /// `options` *must* be the options that were used to create the `func`, and the fields
    ///     *must* be accurate representations.
    /// `func` *must* not be a null pointer, and should satisfy the type it is given
    unsafe fn new(
        cg: CodeGen<'a>,
        options: InstantiateSliceOptions,
        func: ModifySliceInPlaceFn<Target>,
    ) -> InstantiatedSliceModifyInPlace<'a, Target> {
        InstantiatedSliceModifyInPlace { cg, options, func }
    }

    /// Returns the function pointer
    /// Ideally, this should only be used if you're wanting the function pointer so as to use
    /// gdb to print the generated assembly.
    /// # Safety
    /// SAFETY: Note that this pointer is only valid as long as this structure is valid, since
    /// once this structure is dropped, the memory that the function resides at will be freed.
    /// SAFETY: For calling, you must satisfy the safety constraints of
    /// [`Self::call_mut_unchecked_parts`]
    pub fn get_func_unchecked(&self) -> ModifySliceInPlaceFn<Target> {
        self.func
    }

    /// # Panics
    /// If any of the expected constraints are invalid
    pub fn call_mut(&self, data: &mut [Target]) {
        // Thus must be checked in order to guarantee safety
        if let Some(count) = self.options.count {
            assert_eq!(count, data.len());
        }

        // Safety: We forcefully require length to be the same as count
        unsafe {
            self.call_mut_unchecked(data);
        }
    }

    /// # Safety
    /// Safety: `length` must be valid for this function
    ///         if opt.count has a value then `length >= opt.count`
    ///         otherwise, it is legal
    pub unsafe fn call_mut_unchecked(&self, data: &mut [Target]) {
        let length = data.len();
        let data = data.as_mut_ptr();
        // Safety: To construct the func it must have been created
        (self.func)(data, length);
    }

    /// # Safety
    /// Safety: `data` must be non-null
    /// Safety: `data` must be aligned
    /// Safety: `length` must be valid for this function
    ///         if opt.count has a value then `length >= opt.count`
    ///         otherwise it is legal
    /// Safety: `length` must be the number of instances when treating `data` as an array
    ///         That is, all the values at `data + 0`, `data + 1`, ... `data + (length - 1)`
    ///         must be valid
    /// Safety: The pointer passed in must not be mutably aliased elsewhere
    /// Safety: The `data` pointer should be a pointer where the allocation at that point
    ///         is valid to be indexed up to length, aka: don't pair together several allocations
    pub unsafe fn call_mut_unchecked_parts(&self, data: *mut Target, length: usize) {
        (self.func)(data, length)
    }
}

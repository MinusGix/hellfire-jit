use cranelift::{
    codegen::{
        self,
        binemit::{CodeOffset, NullRelocSink, NullStackMapSink, NullTrapSink},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
    prelude::{AbiParam, Block, EntityRef, InstBuilder, IntCC, Type, Value},
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataContext, Linkage, Module};

use crate::{
    jit::JIT,
    util::{make_index_loop, GenerateCode, InitJIT},
    wrappers::ModifyInPlace,
};

pub struct InstantiateSliceOptions {
    /// Is the count known?
    pub count: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum InstantiateError {}

/// Generates a symbol id that is appended after a given prefix
/// in order to generate unique names that are consistent as long as you use the same type
pub(crate) fn generate_name<T: 'static>(prefix: &str) -> String {
    // TODO: We have an issue in that there may be multiple distinct sub functions of different
    // types, so we unfortunately have to do this allocation.
    // We use typeid rather than typename because it has more guarantees on being unique per
    // type

    // TODO: Can we even rely on the type id's debug impl for this? Making a hash of it
    // might be a better choice..
    // I wish cranelift didn't rely on string symbols...
    let id = std::any::TypeId::of::<T>();
    format!("{}{:?}", prefix, id)
}

pub struct FunctionTranslator<'a, 'b> {
    pub(crate) pointer_ty: Type,
    pub(crate) usize_ty: Type,
    /// &mut Target
    pub(crate) data_val: Value,
    pub(crate) builder: &'b mut FunctionBuilder<'a>,
    pub(crate) module: &'a mut JITModule,
}

/// Returns the entry block. The builder is switched to it.
fn make_entry_block(builder: &mut FunctionBuilder) -> Block {
    let entry_block = builder.create_block();
    // Add block params for the function parameters
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);
    entry_block
}

impl<Target: 'static, Actions: GenerateCode + InitJIT + 'static> ModifyInPlace<Target, Actions> {
    /// Instantiate a function over a slice with some statically known data
    pub fn instantiate_slice(
        &self,
        opt: InstantiateSliceOptions,
    ) -> Result<InstantiatedSliceModifyInPlace<Target>, InstantiateError> {
        let mut jit = JIT::new_with(&self.actions);

        let pointer_ty = jit.module.target_config().pointer_type();
        let usize_ty = jit.module.target_config().pointer_type();

        // == Create parameters to function

        // The input data parameter
        jit.ctx
            .func
            .signature
            .params
            .push(AbiParam::new(pointer_ty));

        // The length parameter, which we'll ignore if we have a constant size
        // TODO: if we have a constant size then we don't need to include it at all
        jit.ctx.func.signature.params.push(AbiParam::new(usize_ty));

        // == Build our function

        let mut builder = FunctionBuilder::new(&mut jit.ctx.func, &mut jit.builder_context);

        let entry_block = make_entry_block(&mut builder);

        // Declare parameter variables
        let data_param_index = 0;
        let length_param_index = 1;

        let data_var = declare_and_set_variable_to_parameter(
            &mut builder,
            entry_block,
            data_param_index,
            pointer_ty,
        );
        // We keep this even with a constant size, but we could get rid of it
        // It would be better to have a separate return type for the known count function, though
        // if we did that
        let length_var = declare_and_set_variable_to_parameter(
            &mut builder,
            entry_block,
            length_param_index,
            usize_ty,
        );

        // Declare some variables for MapInPlace slice
        let index_index = 2;
        let index_var = {
            let index_val = builder.ins().iconst(usize_ty, 0);
            let index_var = declare_variable(&mut builder, index_index, usize_ty);
            builder.def_var(index_var, index_val);
            index_var
        };

        // TODO: We probably need to keep track of alignment too?
        let target_size: i64 = std::mem::size_of::<Target>().try_into().unwrap();

        // TODO: Currently we duplicate the closure given to both functions, because I was unable to
        // declare it outside of the function and have the compiler infer working lifetimes..

        // If we have a static count, then generate the loop code based on that rather than length.
        if let Some(count) = opt.count {
            // TODO: Don't unwrap
            let count: i64 = count.try_into().unwrap();

            let count_val = builder.ins().iconst(usize_ty, count);
            make_index_loop(
                &mut builder,
                &mut jit.module,
                data_var,
                index_var,
                count_val,
                target_size,
                move |builder, module, _, element_ptr_val| {
                    let mut trans = FunctionTranslator {
                        pointer_ty,
                        usize_ty,
                        data_val: element_ptr_val,
                        builder,
                        module,
                    };

                    self.actions.generate_code(&mut trans);

                    // Increment the index
                    let index_val = builder.use_var(index_var);
                    let new_index_val = builder.ins().iadd_imm(index_val, 1);
                    builder.def_var(index_var, new_index_val);
                },
            );
        } else {
            make_index_loop(
                &mut builder,
                &mut jit.module,
                data_var,
                index_var,
                length_var,
                target_size,
                move |builder, module, _, element_ptr_val| {
                    let mut trans = FunctionTranslator {
                        pointer_ty,
                        usize_ty,
                        data_val: element_ptr_val,
                        builder,
                        module,
                    };

                    self.actions.generate_code(&mut trans);

                    // Increment the index
                    let index_val = builder.use_var(index_var);
                    let new_index_val = builder.ins().iadd_imm(index_val, 1);
                    builder.def_var(index_var, new_index_val);
                },
            );
        }

        // Return from the function
        builder.ins().return_(&[]);

        // Tell it that we're done with the function
        builder.finalize();

        println!(
            "Target: {}: {:?}",
            jit.module.isa().name(),
            jit.module.isa().triple()
        );
        println!("Function Code: \n{}", jit.ctx.func);

        let mut code = Vec::new();
        jit.ctx
            .compile_and_emit(
                jit.module.isa(),
                &mut code,
                &mut NullRelocSink {},
                &mut NullTrapSink {},
                &mut NullStackMapSink {},
            )
            .unwrap();

        println!("Cool Code:");
        for val in code.iter().rev() {
            print!("{:02X?} ", val);
        }
        println!("");
        // Now we have to declare the function to the jit so that it can be called
        // TODO: This is currently fine since we don't reuse the vm, but we might wish to do that
        // and that would make this name no longer unique. We probably want to include a hash of the
        // options along with the type id
        let name = generate_name::<Target>("slice_instantiate_func");
        let signature = {
            let mut sig = jit.module.make_signature();
            // data
            sig.params.push(AbiParam::new(pointer_ty));
            // length
            sig.params.push(AbiParam::new(usize_ty));

            sig
        };
        // TODO: Don't unwrap
        let id = jit
            .module
            .declare_function(&name, Linkage::Export, &signature)
            .unwrap();
        // Define the function in the jit
        // TODO: Don't unwrap
        let comp = jit
            .module
            .define_function(
                id,
                &mut jit.ctx,
                &mut codegen::binemit::NullTrapSink {},
                &mut codegen::binemit::NullStackMapSink {},
            )
            .unwrap();

        // Clear the context since we're done
        jit.module.clear_context(&mut jit.ctx);

        // Finalize the functions we defines, this resolves any relocations still remaining
        jit.module.finalize_definitions();

        let code = jit.module.get_finalized_function(id);
        let code: unsafe extern "C" fn(*mut Target, usize) = unsafe {
            std::mem::transmute::<*const u8, unsafe extern "C" fn(*mut Target, usize)>(code)
        };

        Ok(InstantiatedSliceModifyInPlace {
            jit,
            options: opt,
            func: code,
            size: comp.size,
        })
    }
}

fn declare_and_set_variable_to_parameter(
    builder: &mut FunctionBuilder,
    block: Block,
    parameter_index: usize,
    typ: Type,
) -> Variable {
    let val = builder.block_params(block)[parameter_index];
    let var = declare_variable(builder, parameter_index, typ);
    builder.def_var(var, val);
    var
}

fn declare_variable(builder: &mut FunctionBuilder, index: usize, typ: Type) -> Variable {
    let var = Variable::new(index);
    builder.declare_var(var, typ);
    var
}

pub struct InstantiatedSliceModifyInPlace<Target> {
    // TODO: Do I actually need to keep around the JIT instance?
    jit: JIT,
    options: InstantiateSliceOptions,
    /// (data, length)
    /// SAFETY: This must not hold a pointer/reference to the value behind it after the completion
    /// of the function.
    /// SAFETY: The pointer must point to a valid allocation where we may safely index into each
    /// entry up to length.
    func: unsafe extern "C" fn(*mut Target, usize),
    pub size: CodeOffset,
}
impl<Target> InstantiatedSliceModifyInPlace<Target> {
    pub(crate) fn get_func(&self) -> unsafe extern "C" fn(*mut Target, usize) {
        self.func
    }

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

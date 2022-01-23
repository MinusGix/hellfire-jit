use std::marker::PhantomData;

use cranelift::prelude::InstBuilder;

use crate::{
    jit::JITInit,
    util::{AsValue, CopyValueOut, CopyValueTo, GenerateCode, GetType, InitJIT, IntegerType},
};

pub struct ModifyInPlace<Target, Actions> {
    pub(crate) actions: Actions,
    _marker: PhantomData<Target>,
}
impl<Target, Actions> ModifyInPlace<Target, Actions> {
    pub fn new(actions: Actions) -> Self {
        Self {
            actions,
            _marker: PhantomData,
        }
    }
}

pub struct Add<Target> {
    pub(crate) amount: Target,
    _marker: PhantomData<Target>,
}
impl<Target> Add<Target> {
    pub fn new(amount: Target) -> Self {
        Self {
            amount,
            _marker: PhantomData,
        }
    }
}
impl<Target> InitJIT for Add<Target> {
    fn init_jit(&self, _: &mut JITInit) {}
}
impl<Target: CopyValueTo + CopyValueOut + AsValue + IntegerType> GenerateCode for Add<Target> {
    fn generate_code(&self, trans: &mut crate::instantiate::FunctionTranslator) {
        // Get the pointer to the value we want to use
        let data_value = trans.data_val;

        let left = Target::copy_value_out(trans, data_value);
        let right = self.amount.as_value(trans);

        let result = trans.builder.ins().iadd(left, right);
        Target::copy_value_to(trans, trans.data_val, result);
    }
}

pub struct Sub<Target> {
    pub(crate) amount: Target,
    _marker: PhantomData<Target>,
}
impl<Target> Sub<Target> {
    pub fn new(amount: Target) -> Self {
        Self {
            amount,
            _marker: PhantomData,
        }
    }
}
impl<Target> InitJIT for Sub<Target> {
    fn init_jit(&self, _: &mut JITInit) {}
}
impl<Target: CopyValueTo + CopyValueOut + AsValue + IntegerType> GenerateCode for Sub<Target> {
    fn generate_code(&self, trans: &mut crate::instantiate::FunctionTranslator) {
        // Get the pointer to the value we want to use
        let data_value = trans.data_val;

        let left = Target::copy_value_out(trans, data_value);
        let right = self.amount.as_value(trans);

        let result = trans.builder.ins().isub(left, right);
        Target::copy_value_to(trans, trans.data_val, result);
    }
}
// The below code is more generic over sub implementations but most Subtracts are for simple values
// and so calling a sub function pointer is too slow for those.
// const SUB_PREFIX: &str = "sub_wrapper_";
// impl<Target: 'static + std::ops::Sub<Target, Output = Target> + AsValue + GetType + Clone> InitJIT
//     for Sub<Target>
// {
//     fn init_jit(&self, init: &mut JITInit) {
//         // Create an instantiation of the function for our specific target
//         let sub_fn: unsafe extern "C" fn(*mut Target, Target) -> Target =
//             sub_identity_wrapper::<Target>;
//         // Convert to general pointer so that it isn't a ZST
//         let sub_fn = sub_fn as *const u8;

//         let name = instantiate::generate_name::<Target>(SUB_PREFIX);
//         init.builder.symbol(&name, sub_fn);
//     }
// }
// impl<
//         Target: 'static + std::ops::Sub<Target, Output = Target> + AsValue + GetType + CopyValue + Clone,
//     > GenerateCode for Sub<Target>
// {
//     fn generate_code(&self, trans: &mut FunctionTranslator) {
//         // Get the type for the right hand side and the return type
//         let amount_type = self.amount.get_type(trans);

//         // TODO: We should make sure this is the same as extern "C"
//         // Create the signature (Target* value, usize length) -> Target
//         let mut signature = trans.module.make_signature();
//         signature.params.push(AbiParam::new(trans.pointer_ty));
//         signature.params.push(AbiParam::new(amount_type));
//         signature.returns.push(AbiParam::new(amount_type));

//         // Get the name of the function so that we can declare it
//         let name = instantiate::generate_name::<Target>(SUB_PREFIX);

//         // TODO: We're probably duplicating this work..
//         // TODO: Don't unwrap
//         let callee = trans
//             .module
//             .declare_function(&name, Linkage::Import, &signature)
//             .unwrap();
//         let local_callee = trans
//             .module
//             .declare_func_in_func(callee, trans.builder.func);

//         let amount_val = self.amount.as_value(trans);
//         let args = [trans.data_val, amount_val];

//         let call = trans.builder.ins().call(local_callee, &args);
//         // The result of the subtraction
//         let value = trans.builder.inst_results(call)[0];
//         Target::copy_value(trans, trans.data_val, value);
//     }
// }
// // TODO: This doesn't really need the *mut pointer
// /// This is a wrapper around the generic types Sub function so that it can be called from jitted
// /// code, since we cannot rely on the Rust calling convention.
// /// Safety: The pointer passed in must be a valid pointer
// unsafe extern "C" fn sub_identity_wrapper<
//     Target: std::ops::Sub<Target, Output = Target> + Clone,
// >(
//     left: *mut Target,
//     right: Target,
// ) -> Target {
//     let left = (*left).clone();
//     left.sub(right)
// }

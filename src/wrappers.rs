use std::marker::PhantomData;

use llama::Value;

use crate::{
    jit::CodeGen,
    util::{AsValue, GenerateCode, IntegerType, StaticGetType},
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
// TODO: We could relax the static get type to a GetType since we do have an instance
// as `amount`
impl<Target: IntegerType + AsValue + StaticGetType> GenerateCode for Add<Target> {
    fn generate_code<'a>(&self, cg: &CodeGen<'a>, data: Value<'a>) -> Result<(), llama::Error> {
        let target_ty = Target::static_get_type(&cg.context)?;
        let amount = self.amount.as_value(cg)?;

        let value = cg.build.load2(target_ty, data, "data_value")?;
        let value = Value::from(value);

        let result = cg.build.add(value, amount, "result")?;
        let result = Value::from(result);

        cg.build.store(result, data)?;

        Ok(())
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
impl<Target: IntegerType + AsValue + StaticGetType> GenerateCode for Sub<Target> {
    fn generate_code<'a>(&self, cg: &CodeGen<'a>, data: Value<'a>) -> Result<(), llama::Error> {
        let target_ty = Target::static_get_type(&cg.context)?;
        let amount = self.amount.as_value(cg)?;

        let value = cg.build.load2(target_ty, data, "data_value")?;
        let value = Value::from(value);

        let result = cg.build.sub(value, amount, "result")?;
        let result = Value::from(result);

        cg.build.store(result, data)?;

        Ok(())
    }
}

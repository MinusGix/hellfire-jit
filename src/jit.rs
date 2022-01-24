use llama::{Builder, Context, ExecutionEngine};

pub struct CodeGen<'ctx> {
    pub(crate) engine: ExecutionEngine<'ctx>,
    pub(crate) build: Builder<'ctx>,
    pub(crate) context: Context<'ctx>,
}

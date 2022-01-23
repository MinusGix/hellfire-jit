use cranelift::{codegen, frontend::FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataContext, Module};

use crate::util::InitJIT;

pub struct JIT {
    /// The functoin builder context
    pub(crate) builder_context: FunctionBuilderContext,

    /// The main cranelife context
    /// Holds the state for codegen
    pub(crate) ctx: codegen::Context,

    /// The data context
    pub(crate) data_ctx: DataContext,

    /// The module which manages the jit'd function
    pub(crate) module: JITModule,
}
impl JIT {
    pub(crate) fn new(builder: JITBuilder) -> JIT {
        let module = JITModule::new(builder);
        JIT {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_ctx: DataContext::new(),
            module,
        }
    }

    pub(crate) fn new_with<A: InitJIT>(a: &A) -> JIT {
        let mut builder = JITBuilder::new(cranelift_module::default_libcall_names());
        let mut jit_init = JITInit {
            builder: &mut builder,
        };

        a.init_jit(&mut jit_init);

        JIT::new(builder)
    }
}

pub struct JITInit<'a> {
    pub(crate) builder: &'a mut JITBuilder,
}

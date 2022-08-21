use crate::{
    compiler::{hir_to_lir::HirToLir, lir::Lir},
    database::Database,
    module::{Module, ModuleDb, ModuleKind},
};

/// Fibers need a context whenever they want to run some expressions. It's used
/// to parameterize the running of the code over the outside world and effects
/// without bleeding implementation details (like salsa) into the code of the
/// VM itself.
pub trait Context {
    fn use_module(&self, module: Module) -> Result<UseResult, String>;
    fn should_continue_running(&self) -> bool;
    fn instruction_executed(&mut self);
}
pub enum UseResult {
    Asset(Vec<u8>),
    Code(Lir),
}

/// Context that can be used when you want to execute some known instructions
/// that are guaranteed not to import other modules.
pub struct DummyContext;
impl Context for DummyContext {
    fn use_module(&self, _: Module) -> Result<UseResult, String> {
        panic!("A dummy context was used for importing a module")
    }

    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(&mut self) {}
}

/// The modular context is a version of the `Context` where several sub-tasks
/// are handled in isolation.
pub struct ModularContext<U: UseProvider, E: ExecutionController> {
    pub use_provider: U,
    pub execution_controller: E,
}
pub trait UseProvider {
    fn use_module(&self, module: Module) -> Result<UseResult, String>;
}
pub trait ExecutionController {
    fn should_continue_running(&self) -> bool;
    fn instruction_executed(&mut self);
}

impl<U: UseProvider, E: ExecutionController> Context for ModularContext<U, E> {
    fn use_module(&self, module: Module) -> Result<UseResult, String> {
        self.use_provider.use_module(module)
    }

    fn should_continue_running(&self) -> bool {
        self.execution_controller.should_continue_running()
    }

    fn instruction_executed(&mut self) {
        self.execution_controller.instruction_executed()
    }
}

/// Uses a salsa database to import modules.
pub struct DbUseProvider<'a> {
    pub db: &'a Database,
}
impl<'a> UseProvider for DbUseProvider<'a> {
    fn use_module(&self, module: Module) -> Result<UseResult, String> {
        match module.kind {
            ModuleKind::Asset => match self.db.get_module_content(module.clone()) {
                Some(bytes) => Ok(UseResult::Asset((*bytes).clone())),
                None => Err(format!("use couldn't import the asset module `{}`", module)),
            },
            ModuleKind::Code => match self.db.lir(module.clone()) {
                Some(lir) => Ok(UseResult::Code((*lir).clone())),
                None => Err(format!("use couldn't import the code module `{}`", module)),
            },
        }
    }
}

/// Limits the execution by the number of executed instructions.
pub struct RunLimitedNumberOfInstructions {
    max_instructions: usize,
    instructions_executed: usize,
}
impl RunLimitedNumberOfInstructions {
    pub fn new(max_instructions: usize) -> Self {
        Self {
            max_instructions,
            instructions_executed: 0,
        }
    }
}
impl ExecutionController for RunLimitedNumberOfInstructions {
    fn should_continue_running(&self) -> bool {
        self.instructions_executed < self.max_instructions
    }

    fn instruction_executed(&mut self) {
        self.instructions_executed += 1;
    }
}

/// Runs forever.
pub struct RunForever;
impl ExecutionController for RunForever {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(&mut self) {}
}

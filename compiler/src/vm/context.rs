use std::sync::Arc;

use crate::{
    compiler::{lir::Lir, mir_to_lir::MirToLir, TracingConfig},
    database::Database,
    module::Module,
};

// VMs and fibers need some of these traits when they run some expressions. This
// allows parameterizing the running of the code over the outside world and
// effects without bleeding implementation details (such as salsa) into the code
// of the VM.

pub trait UseProvider {
    fn use_module(&self, module: Module) -> Option<Arc<Lir>>;
}

pub struct PanickingUseProvider;
impl UseProvider for PanickingUseProvider {
    fn use_module(&self, _: Module) -> Option<Arc<Lir>> {
        panic!()
    }
}

pub struct DbUseProvider<'a> {
    pub db: &'a Database,
    pub tracing: TracingConfig,
}
impl<'a> UseProvider for DbUseProvider<'a> {
    fn use_module(&self, module: Module) -> Option<Arc<Lir>> {
        self.db.lir(module, self.tracing.clone())
    }
}

pub trait ExecutionController {
    fn should_continue_running(&self) -> bool;
    fn instruction_executed(&mut self);
}

pub struct RunForever;
impl ExecutionController for RunForever {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(&mut self) {}
}

pub struct RunLimitedNumberOfInstructions {
    instructions_left: usize,
}
impl RunLimitedNumberOfInstructions {
    pub fn new(max_instructions: usize) -> Self {
        Self {
            instructions_left: max_instructions,
        }
    }
}
impl ExecutionController for RunLimitedNumberOfInstructions {
    fn should_continue_running(&self) -> bool {
        self.instructions_left > 0
    }

    fn instruction_executed(&mut self) {
        if self.instructions_left == 0 {
            panic!();
        }
        self.instructions_left -= 1;
    }
}

pub struct CombiningExecutionController<'a, 'b, A: ExecutionController, B: ExecutionController> {
    a: &'a mut A,
    b: &'b mut B,
}
impl<'a, 'b, A: ExecutionController, B: ExecutionController>
    CombiningExecutionController<'a, 'b, A, B>
{
    pub fn new(a: &'a mut A, b: &'b mut B) -> Self {
        CombiningExecutionController { a, b }
    }
}
impl<'a, 'b, A: ExecutionController, B: ExecutionController> ExecutionController
    for CombiningExecutionController<'a, 'b, A, B>
{
    fn should_continue_running(&self) -> bool {
        self.a.should_continue_running() && self.b.should_continue_running()
    }

    fn instruction_executed(&mut self) {
        self.a.instruction_executed();
        self.b.instruction_executed();
    }
}

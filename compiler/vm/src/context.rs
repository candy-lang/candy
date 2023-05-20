use crate::fiber::InstructionPointer;

pub trait ExecutionController {
    fn should_continue_running(&self) -> bool;
    fn instruction_executed(&mut self, ip: InstructionPointer);
}

pub struct RunForever;
impl ExecutionController for RunForever {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(&mut self, _: InstructionPointer) {}
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

    fn instruction_executed(&mut self, _: InstructionPointer) {
        if self.instructions_left == 0 {
            panic!();
        }
        self.instructions_left -= 1;
    }
}

#[derive(Default)]
pub struct CountingExecutionController {
    pub num_instructions: usize,
}
impl ExecutionController for CountingExecutionController {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(&mut self, _: InstructionPointer) {
        self.num_instructions += 1;
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

    fn instruction_executed(&mut self, ip: InstructionPointer) {
        self.a.instruction_executed(ip);
        self.b.instruction_executed(ip);
    }
}

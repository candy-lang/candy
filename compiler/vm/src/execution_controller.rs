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

macro_rules! impl_execution_controller_tuple {
    ($($name:ident: $lifetime:lifetime $type:ident),+) => {
        impl<$($lifetime),+, $($type),+> ExecutionController for ($(&$lifetime mut $type),+) where $($type: ExecutionController),+ {
            fn should_continue_running(&self) -> bool {
                let ($($name),+) = self;
                $($name.should_continue_running())&&+
            }

            fn instruction_executed(&mut self, ip: InstructionPointer) {
                let ($($name),+) = self;
                $($name.instruction_executed(ip);)+
            }
        }
    };
}
impl_execution_controller_tuple!(c0: 'c0 C0, c1: 'c1 C1);
impl_execution_controller_tuple!(c0: 'c0 C0, c1: 'c1 C1, c2: 'c2 C2);

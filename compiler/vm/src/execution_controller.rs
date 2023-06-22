use crate::{
    fiber::{Fiber, FiberId, InstructionPointer},
    tracer::FiberTracer,
};

pub trait ExecutionController<T: FiberTracer> {
    fn should_continue_running(&self) -> bool;
    fn instruction_executed(&mut self, fiber_id: FiberId, fiber: &Fiber<T>, ip: InstructionPointer);
}

pub struct RunForever;
impl<T: FiberTracer> ExecutionController<T> for RunForever {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(
        &mut self,
        _fiber_id: FiberId,
        _fiber: &Fiber<T>,
        _ip: InstructionPointer,
    ) {
    }
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
impl<T: FiberTracer> ExecutionController<T> for RunLimitedNumberOfInstructions {
    fn should_continue_running(&self) -> bool {
        self.instructions_left > 0
    }

    fn instruction_executed(
        &mut self,
        _fiber_id: FiberId,
        _fiber: &Fiber<T>,
        _ip: InstructionPointer,
    ) {
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
impl<T: FiberTracer> ExecutionController<T> for CountingExecutionController {
    fn should_continue_running(&self) -> bool {
        true
    }

    fn instruction_executed(
        &mut self,
        _fiber_id: FiberId,
        _fiber: &Fiber<T>,
        _ip: InstructionPointer,
    ) {
        self.num_instructions += 1;
    }
}

macro_rules! impl_execution_controller_tuple {
    ($($name:ident: $type:ident),+) => {
        impl<'c, $($type),+, T: FiberTracer> ExecutionController<T> for ($(&'c mut $type),+) where $($type: ExecutionController<T>),+ {
            fn should_continue_running(&self) -> bool {
                let ($($name),+) = self;
                $($name.should_continue_running())&&+
            }

            fn instruction_executed(&mut self, fiber_id: FiberId, fiber: &Fiber<T>, ip: InstructionPointer) {
                let ($($name),+) = self;
                $($name.instruction_executed(fiber_id, fiber, ip);)+
            }
        }
    };
}
impl_execution_controller_tuple!(c0: C0, c1: C1);
impl_execution_controller_tuple!(c0: C0, c1: C1, c2: C2);

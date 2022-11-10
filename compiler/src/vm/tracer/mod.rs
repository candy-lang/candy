pub mod dummy;
pub mod full;
pub mod stack_trace;

use super::{heap::Pointer, ChannelId, FiberId, Heap};

/// An event that happened inside a VM.
#[derive(Clone)]
pub enum VmEvent<'event> {
    FiberCreated {
        fiber: FiberId,
    },
    FiberDone {
        fiber: FiberId,
    },
    FiberPanicked {
        fiber: FiberId,
        panicked_child: Option<FiberId>,
    },
    FiberCanceled {
        fiber: FiberId,
    },
    FiberExecutionStarted {
        fiber: FiberId,
    },
    FiberExecutionEnded {
        fiber: FiberId,
    },
    ChannelCreated {
        channel: ChannelId,
    },
    InFiber {
        fiber: FiberId,
        event: FiberEvent<'event>,
    },
}

/// An event that happened inside a fiber.
#[derive(Clone)]
pub enum FiberEvent<'event> {
    ValueEvaluated {
        expression: Pointer,
        value: Pointer,
        heap: &'event Heap,
    },
    FoundFuzzableClosure {
        definition: Pointer,
        closure: Pointer,
        heap: &'event Heap,
    },
    CallStarted {
        call_site: Pointer,
        closure: Pointer,
        arguments: Vec<Pointer>,
        responsible: Pointer,
        heap: &'event Heap,
    },
    CallEnded {
        return_value: Pointer,
        heap: &'event Heap,
    },
}

pub trait Tracer {
    fn add(&mut self, event: VmEvent);

    fn fiber_created(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberCreated { fiber });
    }
    fn fiber_done(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberDone { fiber });
    }
    fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>) {
        self.add(VmEvent::FiberPanicked {
            fiber,
            panicked_child,
        });
    }
    fn fiber_canceled(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberCanceled { fiber });
    }
    fn fiber_execution_started(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberExecutionStarted { fiber });
    }
    fn fiber_execution_ended(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberExecutionEnded { fiber });
    }
    fn channel_created(&mut self, channel: ChannelId) {
        self.add(VmEvent::ChannelCreated { channel });
    }

    fn for_fiber<'a, 'fiber>(&'a mut self, fiber: FiberId) -> FiberTracer<'fiber>
    where
        Self: Sized,
        'a: 'fiber,
    {
        FiberTracer::<'fiber> {
            tracer: self,
            fiber,
        }
    }
}
pub struct FiberTracer<'fiber> {
    tracer: &'fiber mut dyn Tracer,
    fiber: FiberId,
}
impl<'fiber> FiberTracer<'fiber> {
    fn add(&mut self, event: FiberEvent) {
        self.tracer.add(VmEvent::InFiber {
            fiber: self.fiber,
            event,
        });
    }

    pub fn value_evaluated(&mut self, expression: Pointer, value: Pointer, heap: &Heap) {
        self.add(FiberEvent::ValueEvaluated {
            expression,
            value,
            heap,
        });
    }
    pub fn found_fuzzable_closure(&mut self, definition: Pointer, closure: Pointer, heap: &Heap) {
        self.add(FiberEvent::FoundFuzzableClosure {
            definition,
            closure,
            heap,
        });
    }
    pub fn call_started(
        &mut self,
        call_site: Pointer,
        closure: Pointer,
        args: Vec<Pointer>,
        responsible: Pointer,
        heap: &Heap,
    ) {
        self.add(FiberEvent::CallStarted {
            call_site,
            closure,
            arguments: args,
            responsible,
            heap,
        });
    }
    pub fn call_ended(&mut self, return_value: Pointer, heap: &Heap) {
        self.add(FiberEvent::CallEnded { return_value, heap });
    }
}

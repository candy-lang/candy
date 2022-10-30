pub mod dummy;
pub mod full;
pub mod stack_trace;

use super::{heap::Pointer, ChannelId, FiberId, Heap};
use crate::{compiler::hir::Id, module::Module};

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
    ModuleStarted {
        module: Module,
    },
    ModuleEnded {
        export_map: Pointer,
        heap: &'event Heap,
    },
    ValueEvaluated {
        id: Id,
        value: Pointer,
        heap: &'event Heap,
    },
    FoundFuzzableClosure {
        id: Id,
        closure: Pointer,
        heap: &'event Heap,
    },
    CallStarted {
        id: Id,
        closure: Pointer,
        args: Vec<Pointer>,
        heap: &'event Heap,
    },
    CallEnded {
        return_value: Pointer,
        heap: &'event Heap,
    },
    NeedsStarted {
        id: Id,
        condition: Pointer,
        reason: Pointer,
        heap: &'event Heap,
    },
    NeedsEnded,
}

pub trait Tracer {
    fn add<'event>(&mut self, event: VmEvent<'event>);

    fn for_vm<'a, 'vm>(&'a mut self) -> VmTracer<'vm>
    where
        Self: Sized,
        'a: 'vm,
    {
        VmTracer::<'vm> { tracer: self }
    }
}
pub struct VmTracer<'vm> {
    tracer: &'vm mut dyn Tracer,
}
pub struct FiberTracer<'vm, 'fiber> {
    vm_tracer: &'fiber mut VmTracer<'vm>,
    fiber: FiberId,
}

impl<'vm> VmTracer<'vm> {
    pub fn for_fiber<'fiber>(&'fiber mut self, fiber: FiberId) -> FiberTracer<'vm, 'fiber>
    where
        Self: 'fiber,
    {
        FiberTracer {
            vm_tracer: self,
            fiber,
        }
    }
}

impl<'vm> VmTracer<'vm> {
    fn add<'event>(&mut self, event: VmEvent<'event>) {
        self.tracer.add(event);
    }
}
impl<'vm, 'fiber> FiberTracer<'vm, 'fiber> {
    fn add<'event>(&mut self, event: FiberEvent<'event>) {
        self.vm_tracer.add(VmEvent::InFiber {
            fiber: self.fiber,
            event,
        });
    }
}

impl<'vm> VmTracer<'vm> {
    pub fn fiber_created(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberCreated { fiber });
    }
    pub fn fiber_done(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberDone { fiber });
    }
    pub fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>) {
        self.add(VmEvent::FiberPanicked {
            fiber,
            panicked_child,
        });
    }
    pub fn fiber_canceled(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberCanceled { fiber });
    }
    pub fn fiber_execution_started(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberExecutionStarted { fiber });
    }
    pub fn fiber_execution_ended(&mut self, fiber: FiberId) {
        self.add(VmEvent::FiberExecutionEnded { fiber });
    }
    pub fn channel_created(&mut self, channel: ChannelId) {
        self.add(VmEvent::ChannelCreated { channel });
    }
}
impl<'vm, 'fiber> FiberTracer<'vm, 'fiber> {
    pub fn module_started(&mut self, module: Module) {
        self.add(FiberEvent::ModuleStarted { module });
    }
    pub fn module_ended(&mut self, export_map: Pointer, heap: &Heap) {
        self.add(FiberEvent::ModuleEnded { export_map, heap });
    }
    pub fn value_evaluated(&mut self, id: Id, value: Pointer, heap: &Heap) {
        self.add(FiberEvent::ValueEvaluated { id, value, heap });
    }
    pub fn found_fuzzable_closure(&mut self, id: Id, closure: Pointer, heap: &Heap) {
        self.add(FiberEvent::FoundFuzzableClosure { id, closure, heap });
    }
    pub fn call_started(&mut self, id: Id, closure: Pointer, args: Vec<Pointer>, heap: &Heap) {
        self.add(FiberEvent::CallStarted {
            id,
            closure,
            args,
            heap,
        });
    }
    pub fn call_ended(&mut self, return_value: Pointer, heap: &Heap) {
        self.add(FiberEvent::CallEnded { return_value, heap });
    }
    pub fn needs_started(&mut self, id: Id, condition: Pointer, reason: Pointer, heap: &Heap) {
        self.add(FiberEvent::NeedsStarted {
            id,
            condition,
            reason,
            heap,
        });
    }
    pub fn needs_ended(&mut self) {
        self.add(FiberEvent::NeedsEnded);
    }
}

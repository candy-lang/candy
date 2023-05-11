use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::{Heap, HirId, InlineObject},
    tracer::{stack_trace::Call, FiberEnded, FiberEndedReason, FiberTracer, Tracer},
};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct DebugTracer {
    pub fibers: FxHashMap<FiberId, FiberState>,
}
impl Default for DebugTracer {
    fn default() -> Self {
        Self {
            fibers: FxHashMap::from_iter([(FiberId::root(), FiberState::default())]),
        }
    }
}

impl Tracer for DebugTracer {
    // fn add(&mut self, event: VmEvent) {
    //     match event {
    //         VmEvent::FiberCreated { fiber } => {
    //             self.fibers.insert(fiber, FiberState::default());
    //         }
    //         VmEvent::FiberDone { fiber } => {
    //             self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Done;
    //         }
    //         VmEvent::FiberPanicked { fiber, .. } => {
    //             self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Panicked;
    //         }
    //         VmEvent::FiberCanceled { fiber } => {
    //             self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Canceled;
    //         }
    //     }
    // }

    type ForFiber = FiberDebugTracer;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberDebugTracer::default()
    }
    fn root_fiber_ended(&mut self, ended: FiberEnded<Self::ForFiber>) {
        ended.tracer.drop(ended.heap);
    }
}

#[derive(Debug, Default)]
pub struct FiberState {
    pub status: FiberStatus,
    pub root_locals: Vec<(Id, InlineObject)>,
    pub call_stack: Vec<StackFrame>,
}

#[derive(Debug)]
pub struct StackFrame {
    pub call: Call,
    pub locals: Vec<(Id, InlineObject)>,
}
impl StackFrame {
    fn new(call: Call) -> Self {
        Self {
            call,
            locals: vec![],
        }
    }

    fn dup(&self, heap: &mut Heap) {
        self.call.dup(heap);
        self.locals.iter().for_each(|(_, value)| value.dup(heap));
    }
    fn drop(&self, heap: &mut Heap) {
        self.call.drop(heap);
        self.locals.iter().for_each(|(_, value)| value.drop(heap));
    }
}

#[derive(Debug, Default)]
pub enum FiberStatus {
    #[default]
    Created,
    Done,
    Panicked,
    Canceled,
}
impl From<FiberEndedReason> for FiberStatus {
    fn from(reason: FiberEndedReason) -> Self {
        match reason {
            FiberEndedReason::Finished(_) => Self::Done,
            FiberEndedReason::Panicked(_) => Self::Panicked,
            FiberEndedReason::Canceled => Self::Canceled,
        }
    }
}

// FIXME: inline state
#[derive(Default)]
pub struct FiberDebugTracer(pub FiberState);
impl FiberTracer for FiberDebugTracer {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberDebugTracer::default()
    }
    fn child_fiber_ended(&mut self, ended: FiberEnded<Self>) {
        ended.tracer.drop(ended.heap);
    }

    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        value.dup(heap);
        self.0
            .call_stack
            .last_mut()
            .map(|it| &mut it.locals)
            .unwrap_or(&mut self.0.root_locals)
            .push((expression.get().to_owned(), value));
    }

    fn call_started(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        let call = Call {
            call_site,
            callee,
            arguments,
            responsible,
        };
        call.dup(heap);
        self.0.call_stack.push(StackFrame::new(call));
    }
    fn call_ended(&mut self, heap: &mut Heap, _return_value: InlineObject) {
        self.0.call_stack.pop().unwrap().drop(heap);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap) {
        self.0
            .root_locals
            .iter()
            .for_each(|(_, value)| value.dup(heap));
        for frame in &self.0.call_stack {
            frame.dup(heap);
        }
    }
}

impl FiberDebugTracer {
    fn drop(self, heap: &mut Heap) {
        self.0
            .root_locals
            .into_iter()
            .for_each(|(_, value)| value.drop(heap));
        for frame in self.0.call_stack {
            frame.drop(heap);
        }
    }
}

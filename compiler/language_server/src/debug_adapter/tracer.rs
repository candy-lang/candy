use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::InlineObject,
    tracer::{stack_trace::Call, FiberEvent, Tracer, VmEvent},
};
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct DebugTracer {
    pub new_status_events: Vec<FiberStatusEvent>,
    pub fibers: FxHashMap<FiberId, FiberState>,
}
impl Default for DebugTracer {
    fn default() -> Self {
        Self {
            new_status_events: vec![],
            fibers: FxHashMap::from_iter([(FiberId::root(), FiberState::default())]),
        }
    }
}

#[derive(Debug)]
pub struct FiberState {
    pub status: FiberStatus,
    pub root_locals: Vec<(Id, InlineObject)>,
    pub call_stack: Vec<StackFrame>,
}
impl Default for FiberState {
    fn default() -> Self {
        Self {
            status: FiberStatus::Created,
            root_locals: vec![],
            call_stack: vec![],
        }
    }
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
}

#[derive(Debug)]
pub struct FiberStatusEvent {
    pub fiber: FiberId,
    pub status: FiberStatus,
}

#[derive(Debug)]
pub enum FiberStatus {
    Created,
    Done,
    Panicked,
    Canceled,
}

impl Tracer for DebugTracer {
    fn add(&mut self, event: VmEvent) {
        match event {
            VmEvent::FiberCreated { fiber } => {
                self.fibers.insert(fiber, FiberState::default());
                self.new_status_events.push(FiberStatusEvent {
                    fiber,
                    status: FiberStatus::Created,
                });
            }
            VmEvent::FiberDone { fiber } => {
                self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Done;
                self.new_status_events.push(FiberStatusEvent {
                    fiber,
                    status: FiberStatus::Done,
                });
            }
            VmEvent::FiberPanicked { fiber, .. } => {
                self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Panicked;
                self.new_status_events.push(FiberStatusEvent {
                    fiber,
                    status: FiberStatus::Panicked,
                });
            }
            VmEvent::FiberCanceled { fiber } => {
                self.fibers.get_mut(&fiber).unwrap().status = FiberStatus::Canceled;
                self.new_status_events.push(FiberStatusEvent {
                    fiber,
                    status: FiberStatus::Canceled,
                })
            }
            VmEvent::FiberExecutionStarted { .. } | VmEvent::FiberExecutionEnded { .. } => {}
            VmEvent::ChannelCreated { .. } => {}
            VmEvent::InFiber { fiber, event } => {
                let state = self.fibers.get_mut(&fiber).unwrap();
                match event {
                    FiberEvent::ValueEvaluated {
                        expression, value, ..
                    } => {
                        state
                            .call_stack
                            .last_mut()
                            .map(|it| &mut it.locals)
                            .unwrap_or(&mut state.root_locals)
                            .push((expression.get().to_owned(), value));
                    }
                    FiberEvent::FoundFuzzableClosure { .. } => {}
                    FiberEvent::CallStarted {
                        call_site,
                        callee,
                        arguments,
                        responsible,
                        ..
                    } => {
                        let call = Call {
                            call_site,
                            callee,
                            arguments,
                            responsible,
                        };
                        state.call_stack.push(StackFrame::new(call));
                    }
                    FiberEvent::CallEnded { .. } => {
                        state.call_stack.pop();
                    }
                }
            }
        }
    }
}

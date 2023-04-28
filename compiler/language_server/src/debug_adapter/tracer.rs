use candy_vm::{
    fiber::FiberId,
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
    pub call_stack: Vec<Call>,
}
impl Default for FiberState {
    fn default() -> Self {
        Self {
            status: FiberStatus::Created,
            call_stack: vec![],
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
            VmEvent::InFiber { fiber, event } => match event {
                FiberEvent::ValueEvaluated { .. } => {}
                FiberEvent::FoundFuzzableClosure { .. } => {}
                FiberEvent::CallStarted {
                    call_site,
                    callee,
                    arguments,
                    responsible,
                    ..
                } => {
                    self.fibers.get_mut(&fiber).unwrap().call_stack.push(Call {
                        call_site,
                        callee,
                        arguments,
                        responsible,
                    });
                }
                FiberEvent::CallEnded { .. } => {
                    self.fibers.get_mut(&fiber).unwrap().call_stack.pop();
                }
            },
        }
    }
}

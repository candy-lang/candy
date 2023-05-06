use super::{FiberEvent, Tracer, VmEvent};
use crate::{
    channel::ChannelId,
    fiber::FiberId,
    heap::{Data, Function, Heap, HirId, InlineObject},
};
use itertools::Itertools;
use std::{
    fmt::{self, Debug, Formatter},
    time::Instant,
};

/// A full tracer that saves all events that occur with timestamps.
#[derive(Default)]
pub struct FullTracer {
    pub events: Vec<TimedEvent>,
    pub heap: Heap,
}

#[derive(Clone)]
pub struct TimedEvent {
    pub when: Instant,
    pub event: StoredVmEvent,
}

#[derive(Clone)]
pub enum StoredVmEvent {
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
        event: StoredFiberEvent,
    },
}
#[derive(Clone)]
pub enum StoredFiberEvent {
    ValueEvaluated {
        expression: HirId,
        value: InlineObject,
    },
    FoundFuzzableFunction {
        definition: HirId,
        function: Function,
    },
    CallStarted {
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    },
    CallEnded {
        return_value: InlineObject,
    },
}

impl Tracer for FullTracer {
    fn add(&mut self, event: VmEvent) {
        let event = TimedEvent {
            when: Instant::now(),
            event: self.map_vm_event(event),
        };
        self.events.push(event);
    }
}
impl FullTracer {
    fn map_vm_event(&mut self, event: VmEvent) -> StoredVmEvent {
        match event {
            VmEvent::FiberCreated { fiber } => StoredVmEvent::FiberCreated { fiber },
            VmEvent::FiberDone { fiber } => StoredVmEvent::FiberDone { fiber },
            VmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } => StoredVmEvent::FiberPanicked {
                fiber,
                panicked_child,
            },
            VmEvent::FiberCanceled { fiber } => StoredVmEvent::FiberCanceled { fiber },
            VmEvent::FiberExecutionStarted { fiber } => {
                StoredVmEvent::FiberExecutionStarted { fiber }
            }
            VmEvent::FiberExecutionEnded { fiber } => StoredVmEvent::FiberExecutionEnded { fiber },
            VmEvent::ChannelCreated { channel } => StoredVmEvent::ChannelCreated { channel },
            VmEvent::InFiber { fiber, event } => StoredVmEvent::InFiber {
                fiber,
                event: self.map_fiber_event(event),
            },
        }
    }
    fn map_fiber_event(&mut self, event: FiberEvent) -> StoredFiberEvent {
        match event {
            FiberEvent::ValueEvaluated {
                expression, value, ..
            } => {
                let expression = expression.clone_to_heap(&mut self.heap);
                let value = value.clone_to_heap(&mut self.heap);
                StoredFiberEvent::ValueEvaluated {
                    expression: Data::from(expression).try_into().unwrap(),
                    value,
                }
            }
            FiberEvent::FoundFuzzableFunction {
                definition,
                function,
                ..
            } => {
                let definition = definition.clone_to_heap(&mut self.heap);
                let function = function.clone_to_heap(&mut self.heap);
                StoredFiberEvent::FoundFuzzableFunction {
                    definition: Data::from(definition).try_into().unwrap(),
                    function: Data::from(function).try_into().unwrap(),
                }
            }
            FiberEvent::CallStarted {
                call_site,
                callee,
                arguments,
                responsible,
                ..
            } => {
                let call_site = call_site.clone_to_heap(&mut self.heap);
                let callee = callee.clone_to_heap(&mut self.heap);
                let arguments = arguments
                    .into_iter()
                    .map(|arg| arg.clone_to_heap(&mut self.heap))
                    .collect();
                let responsible = responsible.clone_to_heap(&mut self.heap);
                StoredFiberEvent::CallStarted {
                    call_site: Data::from(call_site).try_into().unwrap(),
                    callee,
                    arguments,
                    responsible: Data::from(responsible).try_into().unwrap(),
                }
            }
            FiberEvent::CallEnded { return_value, .. } => {
                let return_value = return_value.clone_to_heap(&mut self.heap);
                StoredFiberEvent::CallEnded { return_value }
            }
        }
    }
}

impl Debug for FullTracer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let start = self.events.first().map(|event| event.when);
        for event in &self.events {
            writeln!(
                f,
                "{:?} µs: {}",
                event.when.duration_since(start.unwrap()).as_micros(),
                match &event.event {
                    StoredVmEvent::FiberCreated { fiber } => format!("{fiber:?}: created"),
                    StoredVmEvent::FiberDone { fiber } => format!("{fiber:?}: done"),
                    StoredVmEvent::FiberPanicked {
                        fiber,
                        panicked_child,
                    } => format!(
                        "{fiber:?}: panicked{}",
                        if let Some(child) = panicked_child {
                            format!(" because child {child:?} panicked")
                        } else {
                            "".to_string()
                        }
                    ),
                    StoredVmEvent::FiberCanceled { fiber } => format!("{fiber:?}: canceled"),
                    StoredVmEvent::FiberExecutionStarted { fiber } =>
                        format!("{fiber:?}: execution started"),
                    StoredVmEvent::FiberExecutionEnded { fiber } =>
                        format!("{fiber:?}: execution ended"),
                    StoredVmEvent::ChannelCreated { channel } => format!("{channel:?}: created"),
                    StoredVmEvent::InFiber { fiber, event } => format!(
                        "{fiber:?}: {}",
                        match event {
                            StoredFiberEvent::ValueEvaluated { expression, value } =>
                                format!("value {expression} is {value:?}"),
                            StoredFiberEvent::FoundFuzzableFunction { definition, .. } =>
                                format!("found fuzzable function {definition}"),
                            StoredFiberEvent::CallStarted {
                                call_site,
                                callee,
                                arguments,
                                responsible,
                            } => format!(
                                "call started: {callee} {} (call site {call_site}, {responsible} is responsible)",
                                arguments.iter().map(|it| format!("{it:?}")).join(" "),
                            ),
                            StoredFiberEvent::CallEnded { return_value } =>
                                format!("call ended: {return_value:?}"),
                        },
                    ),
                },
            )?;
        }
        Ok(())
    }
}

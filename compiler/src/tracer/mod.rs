// mod full_trace;
pub mod stack_trace;

use crate::{
    compiler::hir::Id,
    module::Module,
    vm::{ChannelId, FiberId, Heap, Pointer},
};
use itertools::Itertools;
use std::{collections::HashMap, fmt, time::Instant};

pub trait Tracer {
    fn fiber_created(&mut self, fiber: FiberId);
    fn fiber_done(&mut self, fiber: FiberId);
    fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>);
    fn fiber_canceled(&mut self, fiber: FiberId);
    fn fiber_execution_started(&mut self, fiber: FiberId);
    fn fiber_execution_ended(&mut self, fiber: FiberId);
    fn channel_created(&mut self, channel: ChannelId);
    fn sent_to_channel(&mut self, value: Pointer, from: FiberId, to: ChannelId);
    fn received_from_channel(&mut self, value: Pointer, from: ChannelId, to: FiberId);

    fn in_fiber_tracer<'a>(&'a mut self, fiber: FiberId) -> Box<dyn InFiberTracer<'a> + 'a>
    where
        Self: 'a;
}

pub trait InFiberTracer<'a> {
    fn module_started(&mut self, module: Module);
    fn module_ended(&mut self, heap: &Heap, export_map: Pointer);
    fn value_evaluated(&mut self, heap: &Heap, id: Id, value: Pointer);
    fn found_fuzzable_closure(&mut self, heap: &Heap, id: Id, closure: Pointer);
    fn call_started(&mut self, heap: &Heap, id: Id, closure: Pointer, args: Vec<Pointer>);
    fn call_ended(&mut self, heap: &Heap, return_value: Pointer);
    fn needs_started(&mut self, heap: &Heap, id: Id, condition: Pointer, reason: Pointer);
    fn needs_ended(&mut self);
}

// A dummy version of the tracer that is used when running known instructions
// without wanting to trace them.

pub struct DummyTracer;
pub struct DummyInFiberTracer;
impl Tracer for DummyTracer {
    fn fiber_created(&mut self, _fiber: FiberId) {}
    fn fiber_done(&mut self, _fiber: FiberId) {}
    fn fiber_panicked(&mut self, _fiber: FiberId, _panicked_child: Option<FiberId>) {}
    fn fiber_canceled(&mut self, _fiber: FiberId) {}
    fn fiber_execution_started(&mut self, _fiber: FiberId) {}
    fn fiber_execution_ended(&mut self, _fiber: FiberId) {}
    fn channel_created(&mut self, _channel: ChannelId) {}
    fn sent_to_channel(&mut self, _value: Pointer, _from: FiberId, _to: ChannelId) {}
    fn received_from_channel(&mut self, _value: Pointer, _from: ChannelId, _to: FiberId) {}

    fn in_fiber_tracer<'a>(&'a mut self, _fiber: FiberId) -> Box<dyn InFiberTracer<'a>>
    where
        Self: 'a,
    {
        Box::new(DummyInFiberTracer)
    }
}
impl<'a> InFiberTracer<'a> for DummyInFiberTracer {
    fn module_started(&mut self, _module: Module) {}
    fn module_ended(&mut self, _heap: &Heap, _export_map: Pointer) {}
    fn value_evaluated(&mut self, _heap: &Heap, _id: Id, _value: Pointer) {}
    fn found_fuzzable_closure(&mut self, _heap: &Heap, _id: Id, _closure: Pointer) {}
    fn call_started(&mut self, _heap: &Heap, _id: Id, _closure: Pointer, _args: Vec<Pointer>) {}
    fn call_ended(&mut self, _heap: &Heap, _return_value: Pointer) {}
    fn needs_started(&mut self, _heap: &Heap, _id: Id, _condition: Pointer, _reason: Pointer) {}
    fn needs_ended(&mut self) {}
}

// A full tracer that saves all events that occur with timestamps.

#[derive(Clone)]
pub struct FullTracer {
    pub events: Vec<TimedEvent>,
    heap: Heap,
    transferred_objects: HashMap<FiberId, HashMap<Pointer, Pointer>>,
}
#[derive(Clone)]
pub struct TimedEvent {
    when: Instant,
    event: Event,
}
#[derive(Clone)]
pub enum Event {
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
    SentToChannel {
        value: Pointer,
        from: FiberId,
        to: ChannelId,
    },
    ReceivedFromChannel {
        value: Pointer,
        from: ChannelId,
        to: FiberId,
    },
    InFiber {
        fiber: FiberId,
        event: InFiberEvent,
    },
}
#[derive(Clone)]
pub enum InFiberEvent {
    ModuleStarted {
        module: Module,
    },
    ModuleEnded {
        export_map: Pointer,
    },
    ValueEvaluated {
        id: Id,
        value: Pointer,
    },
    FoundFuzzableClosure {
        id: Id,
        closure: Pointer,
    },
    CallStarted {
        id: Id,
        closure: Pointer,
        args: Vec<Pointer>,
    },
    CallEnded {
        return_value: Pointer,
    },
    NeedsStarted {
        id: Id,
        condition: Pointer,
        reason: Pointer,
    },
    NeedsEnded,
}

impl FullTracer {
    pub fn new() -> Self {
        Self {
            events: vec![],
            heap: Heap::default(),
            transferred_objects: HashMap::new(),
        }
    }
    fn push(&mut self, data: Event) {
        self.events.push(TimedEvent {
            when: Instant::now(),
            event: data,
        });
    }
    fn import_from_fiber_heap(&mut self, fiber: FiberId, heap: &Heap, value: Pointer) -> Pointer {
        let address_map = self
            .transferred_objects
            .entry(fiber)
            .or_insert_with(HashMap::new);
        heap.clone_single_to_other_heap_with_existing_mapping(&mut self.heap, value, address_map)
    }
}
impl Tracer for FullTracer {
    fn fiber_created(&mut self, fiber: FiberId) {
        self.push(Event::FiberCreated { fiber });
    }
    fn fiber_done(&mut self, fiber: FiberId) {
        self.push(Event::FiberDone { fiber });
    }
    fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>) {
        self.push(Event::FiberPanicked {
            fiber,
            panicked_child,
        });
    }
    fn fiber_canceled(&mut self, fiber: FiberId) {
        self.push(Event::FiberCanceled { fiber });
    }
    fn fiber_execution_started(&mut self, fiber: FiberId) {
        self.push(Event::FiberExecutionStarted { fiber });
    }
    fn fiber_execution_ended(&mut self, fiber: FiberId) {
        self.push(Event::FiberExecutionEnded { fiber });
    }
    fn channel_created(&mut self, channel: ChannelId) {
        self.push(Event::ChannelCreated { channel });
    }
    fn sent_to_channel(&mut self, value: Pointer, from: FiberId, to: ChannelId) {
        self.push(Event::SentToChannel { value, from, to });
    }
    fn received_from_channel(&mut self, value: Pointer, from: ChannelId, to: FiberId) {
        self.push(Event::ReceivedFromChannel { value, from, to });
    }

    fn in_fiber_tracer<'a>(&'a mut self, fiber: FiberId) -> Box<dyn InFiberTracer<'a> + 'a>
    where
        Self: 'a,
    {
        Box::new(FullInFiberTracer {
            tracer: self,
            fiber,
        })
    }
}

pub struct FullInFiberTracer<'a> {
    tracer: &'a mut FullTracer,
    fiber: FiberId,
}
impl<'a> FullInFiberTracer<'a> {
    fn import_from_fiber_heap(&mut self, heap: &Heap, value: Pointer) -> Pointer {
        self.tracer.import_from_fiber_heap(self.fiber, heap, value)
    }
    fn push(&mut self, event: InFiberEvent) {
        self.tracer.push(Event::InFiber {
            fiber: self.fiber,
            event,
        });
    }
}
impl<'a> InFiberTracer<'a> for FullInFiberTracer<'a> {
    fn module_started(&mut self, module: Module) {
        self.push(InFiberEvent::ModuleStarted { module });
    }
    fn module_ended(&mut self, heap: &Heap, export_map: Pointer) {
        let export_map = self.import_from_fiber_heap(heap, export_map);
        self.push(InFiberEvent::ModuleEnded { export_map });
    }
    fn value_evaluated(&mut self, heap: &Heap, id: Id, value: Pointer) {
        let value = self.import_from_fiber_heap(heap, value);
        self.push(InFiberEvent::ValueEvaluated { id, value });
    }
    fn found_fuzzable_closure(&mut self, heap: &Heap, id: Id, closure: Pointer) {
        let closure = self.import_from_fiber_heap(heap, closure);
        self.push(InFiberEvent::FoundFuzzableClosure { id, closure });
    }
    fn call_started(&mut self, heap: &Heap, id: Id, closure: Pointer, args: Vec<Pointer>) {
        let closure = self.import_from_fiber_heap(heap, closure);
        let args = args
            .into_iter()
            .map(|arg| self.import_from_fiber_heap(heap, arg))
            .collect();
        self.push(InFiberEvent::CallStarted { id, closure, args });
    }
    fn call_ended(&mut self, heap: &Heap, return_value: Pointer) {
        let return_value = self.import_from_fiber_heap(heap, return_value);
        self.push(InFiberEvent::CallEnded { return_value });
    }
    fn needs_started(&mut self, heap: &Heap, id: Id, condition: Pointer, reason: Pointer) {
        let condition = self.import_from_fiber_heap(heap, condition);
        let reason = self.import_from_fiber_heap(heap, reason);
        self.push(InFiberEvent::NeedsStarted {
            id,
            condition,
            reason,
        });
    }
    fn needs_ended(&mut self) {
        self.push(InFiberEvent::NeedsEnded);
    }
}

impl fmt::Debug for FullTracer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let start = self.events.first().map(|event| event.when);
        for event in &self.events {
            writeln!(
                f,
                "{:?} us: {}",
                event.when.duration_since(start.unwrap()).as_micros(),
                match &event.event {
                    Event::FiberCreated { fiber } => format!("{fiber:?}: created"),
                    Event::FiberDone { fiber } => format!("{fiber:?}: done"),
                    Event::FiberPanicked {
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
                    Event::FiberCanceled { fiber } => format!("{fiber:?}: canceled"),
                    Event::FiberExecutionStarted { fiber } =>
                        format!("{fiber:?}: execution started"),
                    Event::FiberExecutionEnded { fiber } => format!("{fiber:?}: execution ended"),
                    Event::ChannelCreated { channel } => format!("{channel:?}: created"),
                    Event::SentToChannel { value, from, to } =>
                        format!("{from:?} sent {} to {to:?}", value.format(&self.heap)),
                    Event::ReceivedFromChannel { value, from, to } =>
                        format!("{to:?} received {} from {from:?}", value.format(&self.heap)),
                    Event::InFiber { fiber, event } => format!(
                        "{fiber:?}: {}",
                        match event {
                            InFiberEvent::ModuleStarted { module } =>
                                format!("module {module} started"),
                            InFiberEvent::ModuleEnded { export_map } => format!(
                                "module ended and exported {}",
                                export_map.format(&self.heap)
                            ),
                            InFiberEvent::ValueEvaluated { id, value } =>
                                format!("value {id} is {}", value.format(&self.heap)),
                            InFiberEvent::FoundFuzzableClosure { id, .. } =>
                                format!("found fuzzable closure {id}"),
                            InFiberEvent::CallStarted { id, closure, args } => format!(
                                "call {id} started: {} {}",
                                closure.format(&self.heap),
                                args.iter().map(|arg| arg.format(&self.heap)).join(" ")
                            ),
                            InFiberEvent::CallEnded { return_value } =>
                                format!("call ended: {}", return_value.format(&self.heap)),
                            InFiberEvent::NeedsStarted {
                                id,
                                condition,
                                reason,
                            } => format!(
                                "needs {id} started: needs {} {}",
                                condition.format(&self.heap),
                                reason.format(&self.heap)
                            ),
                            InFiberEvent::NeedsEnded => "needs ended".to_string(),
                        }
                    ),
                }
            )?;
        }
        Ok(())
    }
}

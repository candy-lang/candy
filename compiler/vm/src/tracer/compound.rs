use super::{FiberTracer, TracedFiberEnded, Tracer};
use crate::{
    channel::ChannelId,
    fiber::FiberId,
    heap::{Function, Heap, HirId, InlineObject},
};

#[derive(Default)]
pub struct CompoundTracer<T0: Tracer, T1: Tracer> {
    pub tracer0: T0,
    pub tracer1: T1,
}
impl<T0: Tracer, T1: Tracer> Tracer for CompoundTracer<T0, T1> {
    type ForFiber = CompoundFiberTracer<T0::ForFiber, T1::ForFiber>;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        CompoundFiberTracer {
            tracer0: self.tracer0.root_fiber_created(),
            tracer1: self.tracer1.root_fiber_created(),
        }
    }
    fn root_fiber_ended(&mut self, ended: TracedFiberEnded<Self::ForFiber>) {
        self.tracer0.root_fiber_ended(TracedFiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer0,
            reason: ended.reason.clone(),
        });
        self.tracer1.root_fiber_ended(TracedFiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer1,
            reason: ended.reason,
        });
    }

    fn fiber_execution_started(&mut self, fiber: FiberId) {
        self.tracer0.fiber_execution_started(fiber);
        self.tracer1.fiber_execution_started(fiber);
    }
    fn fiber_execution_ended(&mut self, fiber: FiberId) {
        self.tracer0.fiber_execution_ended(fiber);
        self.tracer1.fiber_execution_ended(fiber);
    }

    fn channel_created(&mut self, channel: ChannelId) {
        self.tracer0.channel_created(channel);
        self.tracer1.channel_created(channel);
    }
}

#[derive(Default)]
pub struct CompoundFiberTracer<T0: FiberTracer, T1: FiberTracer> {
    tracer0: T0,
    tracer1: T1,
}
impl<T0: FiberTracer, T1: FiberTracer> FiberTracer for CompoundFiberTracer<T0, T1> {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        CompoundFiberTracer {
            tracer0: self.tracer0.child_fiber_created(_child),
            tracer1: self.tracer1.child_fiber_created(_child),
        }
    }
    fn child_fiber_ended(&mut self, ended: TracedFiberEnded<Self>) {
        self.tracer0.child_fiber_ended(TracedFiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer0,
            reason: ended.reason.clone(),
        });
        self.tracer1.child_fiber_ended(TracedFiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer1,
            reason: ended.reason,
        });
    }

    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        self.tracer0.value_evaluated(heap, expression, value);
        self.tracer1.value_evaluated(heap, expression, value);
    }

    fn found_fuzzable_function(&mut self, heap: &mut Heap, definition: HirId, function: Function) {
        self.tracer0
            .found_fuzzable_function(heap, definition, function);
        self.tracer1
            .found_fuzzable_function(heap, definition, function);
    }

    fn call_started(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        self.tracer0
            .call_started(heap, call_site, callee, arguments.clone(), responsible);
        self.tracer1
            .call_started(heap, call_site, callee, arguments, responsible);
    }
    fn call_ended(&mut self, heap: &mut Heap, return_value: InlineObject) {
        self.tracer0.call_ended(heap, return_value);
        self.tracer1.call_ended(heap, return_value);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap) {
        self.tracer0.dup_all_stored_objects(heap);
        self.tracer1.dup_all_stored_objects(heap);
    }
}

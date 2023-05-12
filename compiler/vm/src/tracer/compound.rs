use super::{FiberEnded, FiberTracer, Tracer};
use crate::{
    channel::ChannelId,
    fiber::FiberId,
    heap::{Function, Heap, HirId, InlineObject},
};
use std::marker::PhantomData;

#[derive(Default)]
pub struct CompoundTracer<'h, T0: Tracer<'h>, T1: Tracer<'h>> {
    pub tracer0: T0,
    pub tracer1: T1,
    phantom: PhantomData<&'h ()>,
}
impl<'h, T0: Tracer<'h>, T1: Tracer<'h>> CompoundTracer<'h, T0, T1> {
    pub fn new(tracer0: T0, tracer1: T1) -> Self {
        Self {
            tracer0,
            tracer1,
            phantom: PhantomData,
        }
    }
}
impl<'h, T0: Tracer<'h>, T1: Tracer<'h>> Tracer<'h> for CompoundTracer<'h, T0, T1> {
    type ForFiber = CompoundFiberTracer<'h, T0::ForFiber, T1::ForFiber>;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        CompoundFiberTracer {
            tracer0: self.tracer0.root_fiber_created(),
            tracer1: self.tracer1.root_fiber_created(),
            phantom: PhantomData,
        }
    }
    fn root_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self::ForFiber>) {
        self.tracer0.root_fiber_ended(FiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer0,
            reason: ended.reason.clone(),
        });
        self.tracer1.root_fiber_ended(FiberEnded {
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
pub struct CompoundFiberTracer<'h, T0: FiberTracer<'h>, T1: FiberTracer<'h>> {
    tracer0: T0,
    tracer1: T1,
    phantom: PhantomData<&'h ()>,
}
impl<'h, T0: FiberTracer<'h>, T1: FiberTracer<'h>> FiberTracer<'h>
    for CompoundFiberTracer<'h, T0, T1>
{
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        CompoundFiberTracer {
            tracer0: self.tracer0.child_fiber_created(_child),
            tracer1: self.tracer1.child_fiber_created(_child),
            phantom: PhantomData,
        }
    }
    fn child_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self>) {
        self.tracer0.child_fiber_ended(FiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer0,
            reason: ended.reason.clone(),
        });
        self.tracer1.child_fiber_ended(FiberEnded {
            id: ended.id,
            heap: ended.heap,
            tracer: ended.tracer.tracer1,
            reason: ended.reason,
        });
    }

    fn value_evaluated(
        &mut self,
        heap: &mut Heap<'h>,
        expression: HirId<'h>,
        value: InlineObject<'h>,
    ) {
        self.tracer0.value_evaluated(heap, expression, value);
        self.tracer1.value_evaluated(heap, expression, value);
    }

    fn found_fuzzable_function(
        &mut self,
        heap: &mut Heap<'h>,
        definition: HirId<'h>,
        function: Function<'h>,
    ) {
        self.tracer0
            .found_fuzzable_function(heap, definition, function);
        self.tracer1
            .found_fuzzable_function(heap, definition, function);
    }

    fn call_started(
        &mut self,
        heap: &mut Heap<'h>,
        call_site: HirId<'h>,
        callee: InlineObject<'h>,
        arguments: Vec<InlineObject<'h>>,
        responsible: HirId<'h>,
    ) {
        self.tracer0
            .call_started(heap, call_site, callee, arguments.clone(), responsible);
        self.tracer1
            .call_started(heap, call_site, callee, arguments, responsible);
    }
    fn call_ended(&mut self, heap: &mut Heap<'h>, return_value: InlineObject<'h>) {
        self.tracer0.call_ended(heap, return_value);
        self.tracer1.call_ended(heap, return_value);
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap<'h>) {
        self.tracer0.dup_all_stored_objects(heap);
        self.tracer1.dup_all_stored_objects(heap);
    }
}

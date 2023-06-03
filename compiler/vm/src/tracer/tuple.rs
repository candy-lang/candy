use super::{FiberTracer, TracedFiberEnded, Tracer};
use crate::{
    channel::ChannelId,
    fiber::FiberId,
    heap::{Function, Heap, HirId, InlineObject},
};
use impl_trait_for_tuples::impl_for_tuples;

#[impl_for_tuples(2, 3)]
impl Tracer for Tuple {
    for_tuples!( type ForFiber = ( #(Tuple::ForFiber),* ); );

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        for_tuples!( ( #(Tuple.root_fiber_created()),* ) );
    }
    #[allow(clippy::redundant_clone)] // PERF: Avoid clone for last tuple element
    fn root_fiber_ended(&mut self, ended: TracedFiberEnded<Self::ForFiber>) {
        for_tuples!(
            #(Tuple::root_fiber_ended(
                &mut self.Tuple,
                TracedFiberEnded {
                    id: ended.id,
                    heap: ended.heap,
                    tracer: ended.tracer.Tuple,
                    reason: ended.reason.clone(),
                },
            );)*
        );
    }

    fn fiber_execution_started(&mut self, fiber: FiberId) {
        for_tuples!( #(Tuple.fiber_execution_started(fiber);)* );
    }
    fn fiber_execution_ended(&mut self, fiber: FiberId) {
        for_tuples!( #(Tuple.fiber_execution_ended(fiber);)* );
    }

    fn channel_created(&mut self, channel: ChannelId) {
        for_tuples!( #(Tuple.channel_created(channel);)* );
    }
}

#[impl_for_tuples(2, 3)]
impl FiberTracer for Tuple {
    fn child_fiber_created(&mut self, child: FiberId) -> Self {
        for_tuples!( ( #(Tuple.child_fiber_created(child)),* ) );
    }
    #[allow(clippy::redundant_clone)] // PERF: Avoid clone for last tuple element
    fn child_fiber_ended(&mut self, ended: TracedFiberEnded<Self>) {
        for_tuples!(
            #(Tuple::child_fiber_ended(
                &mut self.Tuple,
                TracedFiberEnded {
                    id: ended.id,
                    heap: ended.heap,
                    tracer: ended.tracer.Tuple,
                    reason: ended.reason.clone(),
                },
            );)*
        );
    }

    fn value_evaluated(&mut self, heap: &mut Heap, expression: HirId, value: InlineObject) {
        for_tuples!( #(Tuple.value_evaluated(heap, expression, value);)* );
    }

    fn found_fuzzable_function(&mut self, heap: &mut Heap, definition: HirId, function: Function) {
        for_tuples!( #(Tuple.found_fuzzable_function(heap, definition, function);)* );
    }

    #[allow(clippy::redundant_clone)] // PERF: Avoid clone for last tuple element
    fn call_started(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        for_tuples!( #(Tuple.call_started(heap, call_site, callee, arguments.clone(), responsible);)* );
    }
    fn call_ended(&mut self, heap: &mut Heap, return_value: InlineObject) {
        for_tuples!( #(Tuple.call_ended(heap, return_value);)* );
    }

    fn dup_all_stored_objects(&self, heap: &mut Heap) {
        for_tuples!( #(Tuple.dup_all_stored_objects(heap);)* );
    }
}

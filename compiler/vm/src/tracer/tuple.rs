use super::Tracer;
use crate::heap::{Function, Heap, HirId, InlineObject};
use impl_trait_for_tuples::impl_for_tuples;

#[impl_for_tuples(2, 3)]
impl Tracer for Tuple {
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
    #[allow(clippy::redundant_clone)] // PERF: Avoid clone for last tuple element
    fn tail_call(
        &mut self,
        heap: &mut Heap,
        call_site: HirId,
        callee: InlineObject,
        arguments: Vec<InlineObject>,
        responsible: HirId,
    ) {
        for_tuples!( #(Tuple.tail_call(heap, call_site, callee, arguments.clone(), responsible);)* );
    }
}

pub use self::dummy::DummyTracer;
use crate::heap::{Function, Heap, HirId, InlineObject};

mod dummy;
pub mod evaluated_values;
pub mod stack_trace;
pub mod tuple;

pub trait Tracer {
    fn value_evaluated(&mut self, _heap: &mut Heap, _expression: HirId, _value: InlineObject) {}

    fn found_fuzzable_function(
        &mut self,
        _heap: &mut Heap,
        _definition: HirId,
        _function: Function,
    ) {
    }

    fn call_started(
        &mut self,
        _heap: &mut Heap,
        _call_site: HirId,
        _callee: InlineObject,
        _arguments: Vec<InlineObject>,
        _responsible: HirId,
    ) {
    }
    fn call_ended(&mut self, _heap: &mut Heap, _return_value: InlineObject) {}
}

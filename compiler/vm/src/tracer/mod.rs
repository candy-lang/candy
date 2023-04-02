use crate::channel::ChannelId;

pub use self::dummy::DummyTracer;
use super::{
    fiber::FiberId,
    heap::{Heap, Pointer},
};

mod dummy;
pub mod full;
pub mod stack_trace;

pub trait Tracer {
    fn fiber_created(&mut self, fiber: FiberId) {}
    fn fiber_done(&mut self, fiber: FiberId) {}
    fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>) {}
    fn fiber_canceled(&mut self, fiber: FiberId) {}
    fn fiber_execution_started(&mut self, fiber: FiberId) {}
    fn fiber_execution_ended(&mut self, fiber: FiberId) {}
    fn channel_created(&mut self, channel: ChannelId) {}

    fn value_evaluated(
        &mut self,
        fiber: FiberId,
        expression: Pointer,
        value: Pointer,
        heap: &mut Heap,
    ) {
    }
    fn found_fuzzable_closure(
        &mut self,
        fiber: FiberId,
        definition: Pointer,
        closure: Pointer,
        heap: &Heap,
    ) {
    }
    fn call_started(
        &mut self,
        call_site: Pointer,
        callee: Pointer,
        args: Vec<Pointer>,
        responsible: Pointer,
        heap: &Heap,
    ) {
    }
    fn call_ended(&mut self, return_value: Pointer, heap: &Heap) {}
}

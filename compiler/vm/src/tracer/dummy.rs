use crate::{
    channel::ChannelId,
    fiber::FiberId,
    heap::{Heap, Pointer},
};

use super::{FiberTracer, Tracer};

/// A tracer that remembers nothing.
#[derive(Default)]
pub struct DummyTracer;
impl Tracer for DummyTracer {
    type ForFiber = DummyTracer;

    fn fiber_created(&mut self, _: FiberId) {}
    fn fiber_done(&mut self, _: FiberId) {}
    fn fiber_panicked(&mut self, _: FiberId, _: Option<FiberId>) {}
    fn fiber_canceled(&mut self, _: FiberId) {}
    fn fiber_execution_started(&mut self, _: FiberId) {}
    fn fiber_execution_ended(&mut self, _: FiberId) {}
    fn channel_created(&mut self, _: ChannelId) {}
    fn tracer_for_fiber(&mut self, _: FiberId) -> DummyTracer {}
}
impl FiberTracer for DummyTracer {
    fn value_evaluated(&mut self, _: Pointer, _: Pointer, _: &mut Heap) {}
    fn found_fuzzable_closure(&mut self, _: Pointer, _: Pointer, _: &mut Heap) {}
    fn call_started(&mut self, _: Pointer, _: Pointer, _: Vec<Pointer>, _: Pointer, _: &mut Heap) {}
    fn call_ended(&mut self, _: Pointer, _: &mut Heap) {}
}

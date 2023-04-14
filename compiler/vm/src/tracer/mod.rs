use crate::channel::ChannelId;

pub use self::dummy::DummyTracer;
use super::{
    fiber::FiberId,
    heap::{Heap, Pointer},
};

mod dummy;
pub mod stack_trace;

pub trait Tracer {
    type ForFiber: FiberTracer;

    fn fiber_created(&mut self, id: FiberId);
    fn fiber_done(&mut self, id: FiberId);
    fn fiber_panicked(&mut self, id: FiberId, panicked_child: Option<FiberId>);
    fn fiber_canceled(&mut self, id: FiberId);
    fn fiber_execution_started(&mut self, id: FiberId);
    fn fiber_execution_ended(&mut self, id: FiberId);
    fn channel_created(&mut self, channel: ChannelId);

    fn tracer_for_fiber(&mut self, id: FiberId) -> Self::ForFiber;
    fn integrate_fiber_tracer(&mut self, tracer: Self::ForFiber, from: &Heap, to: &mut Heap);
}

pub trait FiberTracer {
    fn value_evaluated(&mut self, expression: Pointer, value: Pointer, heap: &mut Heap);
    fn found_fuzzable_closure(&mut self, definition: Pointer, closure: Pointer, heap: &mut Heap);
    fn call_started(
        &mut self,
        call_site: Pointer,
        callee: Pointer,
        args: Vec<Pointer>,
        responsible: Pointer,
        heap: &mut Heap,
    );
    fn call_ended(&mut self, return_value: Pointer, heap: &mut Heap);
}

pub use self::dummy::DummyTracer;
use super::{fiber::FiberId, heap::Heap};
use crate::{
    channel::ChannelId,
    fiber::ExecutionPanicked,
    heap::{Function, HirId, InlineObject},
};

pub mod compound;
mod dummy;
pub mod evaluated_values;
pub mod stack_trace;

pub trait Tracer {
    type ForFiber: FiberTracer;

    fn root_fiber_created(&mut self) -> Self::ForFiber;
    fn root_fiber_ended(&mut self, _ended: FiberEnded<Self::ForFiber>) {}

    fn fiber_execution_started(&mut self, _fiber: FiberId) {}
    fn fiber_execution_ended(&mut self, _fiber: FiberId) {}

    fn channel_created(&mut self, _channel: ChannelId) {}
}

pub trait FiberTracer
where
    Self: Sized,
{
    fn child_fiber_created(&mut self, child: FiberId) -> Self;
    fn child_fiber_ended(&mut self, _ended: FiberEnded<Self>) {}

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

pub struct FiberEnded<'h, T: FiberTracer> {
    pub id: FiberId,
    pub heap: &'h mut Heap,
    pub tracer: T,
    pub reason: FiberEndedReason,
}
#[derive(Clone)]
pub enum FiberEndedReason {
    Finished(InlineObject),
    Panicked(ExecutionPanicked),
    Canceled,
}

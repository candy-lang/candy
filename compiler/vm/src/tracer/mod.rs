pub use self::dummy::DummyTracer;
use super::{fiber::FiberId, heap::Heap};
use crate::{
    channel::ChannelId,
    fiber::Panic,
    heap::{Function, HirId, InlineObject},
};

pub mod compound;
mod dummy;
pub mod evaluated_values;
pub mod stack_trace;

pub trait Tracer<'h> {
    type ForFiber: FiberTracer<'h>;

    fn root_fiber_created(&mut self) -> Self::ForFiber;
    fn root_fiber_ended<'a>(&mut self, _ended: FiberEnded<'a, 'h, Self::ForFiber>) {}

    fn fiber_execution_started(&mut self, _fiber: FiberId) {}
    fn fiber_execution_ended(&mut self, _fiber: FiberId) {}

    fn channel_created(&mut self, _channel: ChannelId) {}
}

pub trait FiberTracer<'h>
where
    Self: Sized,
{
    fn child_fiber_created(&mut self, child: FiberId) -> Self;
    fn child_fiber_ended<'a>(&mut self, _ended: FiberEnded<'a, 'h, Self>) {}

    fn value_evaluated(
        &mut self,
        _heap: &mut Heap<'h>,
        _expression: HirId<'h>,
        _value: InlineObject<'h>,
    ) {
    }

    fn found_fuzzable_function(
        &mut self,
        _heap: &mut Heap<'h>,
        _definition: HirId<'h>,
        _function: Function<'h>,
    ) {
    }

    fn call_started(
        &mut self,
        _heap: &mut Heap<'h>,
        _call_site: HirId<'h>,
        _callee: InlineObject<'h>,
        _arguments: Vec<InlineObject<'h>>,
        _responsible: HirId<'h>,
    ) {
    }
    fn call_ended(&mut self, _heap: &mut Heap<'h>, _return_value: InlineObject<'h>) {}

    fn dup_all_stored_objects(&self, _heap: &mut Heap<'h>);
}

pub struct FiberEnded<'a, 'h, T: FiberTracer<'h>> {
    pub id: FiberId,
    pub heap: &'a mut Heap<'h>,
    pub tracer: T,
    pub reason: FiberEndedReason<'h>,
}
#[derive(Clone)]
pub enum FiberEndedReason<'h> {
    Finished(InlineObject<'h>),
    Panicked(Panic),
    Canceled,
}

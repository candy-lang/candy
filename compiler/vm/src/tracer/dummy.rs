use super::Tracer;

/// A dummy version of the tracer that remembers nothing.
#[derive(Default)]
pub struct DummyTracer;
impl Tracer for DummyTracer {
    fn fiber_created(&mut self, fiber: crate::fiber::FiberId) {}

    fn fiber_done(&mut self, fiber: crate::fiber::FiberId) {}

    fn fiber_panicked(
        &mut self,
        fiber: crate::fiber::FiberId,
        panicked_child: Option<crate::fiber::FiberId>,
    ) {
    }

    fn fiber_canceled(&mut self, fiber: crate::fiber::FiberId) {}

    fn fiber_execution_started(&mut self, fiber: crate::fiber::FiberId) {}

    fn fiber_execution_ended(&mut self, fiber: crate::fiber::FiberId) {}

    fn channel_created(&mut self, channel: crate::channel::ChannelId) {}

    fn value_evaluated(
        &mut self,
        fiber: crate::fiber::FiberId,
        expression: crate::heap::Pointer,
        value: crate::heap::Pointer,
        heap: &mut crate::heap::Heap,
    ) {
    }

    fn found_fuzzable_closure(
        &mut self,
        fiber: crate::fiber::FiberId,
        definition: crate::heap::Pointer,
        closure: crate::heap::Pointer,
        heap: &crate::heap::Heap,
    ) {
    }

    fn call_started(
        &mut self,
        call_site: crate::heap::Pointer,
        callee: crate::heap::Pointer,
        args: Vec<crate::heap::Pointer>,
        responsible: crate::heap::Pointer,
        heap: &crate::heap::Heap,
    ) {
    }

    fn call_ended(&mut self, return_value: crate::heap::Pointer, heap: &crate::heap::Heap) {}
}

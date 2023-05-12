use super::{FiberTracer, Tracer};
use crate::{fiber::FiberId, heap::Heap};

/// A dummy version of the tracer that remembers nothing.
#[derive(Default)]
pub struct DummyTracer;
impl<'h> Tracer<'h> for DummyTracer {
    type ForFiber = DummyFiberTracer;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        DummyFiberTracer
    }
}

#[derive(Default)]
pub struct DummyFiberTracer;
impl<'h> FiberTracer<'h> for DummyFiberTracer {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        DummyFiberTracer
    }

    fn dup_all_stored_objects(&self, _heap: &mut Heap<'h>) {}
}

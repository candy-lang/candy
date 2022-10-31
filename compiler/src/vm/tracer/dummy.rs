use super::{Tracer, VmEvent};

/// A dummy version of the tracer that remembers nothing.
pub struct DummyTracer;
impl Tracer for DummyTracer {
    fn add(&mut self, _: VmEvent) {}
}

use super::Tracer;

/// A dummy version of the tracer that remembers nothing.
#[derive(Default)]
pub struct DummyTracer;
impl Tracer for DummyTracer {}

use candy_vm::{channel::ChannelId, fiber::FiberId, heap::Heap, tracer::Tracer};

pub struct ConstantTracer;

impl Tracer for ConstantTracer {
    type ForFiber = ConstantTracerForFiber;

    fn fiber_created(&mut self, _: FiberId) {}
    fn fiber_done(&mut self, _: FiberId) {}
    fn fiber_panicked(&mut self, _: FiberId, _: Option<FiberId>) {}
    fn fiber_canceled(&mut self, _: FiberId) {}
    fn fiber_execution_started(&mut self, _: FiberId) {}
    fn fiber_execution_ended(&mut self, _: FiberId) {}
    fn channel_created(&mut self, _: ChannelId) {}

    fn tracer_for_fiber(&mut self, id: FiberId) -> Self::ForFiber {
        if id == FiberId::root() {
            ConstantTracerForFiber::Trace
        } else {
            ConstantTracerForFiber::NoTracing
        }
    }

    fn integrate_fiber_tracer(&mut self, tracer: Self::ForFiber, from: &Heap, to: &mut Heap) {
        assert!(matches!(tracer, ConstantTracerForFiber::NoTracing))
    }
}

enum ConstantTracerForFiber {
    NoTracing,
    Trace,
}

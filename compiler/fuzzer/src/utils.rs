use rustc_hash::FxHashMap;

use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::{Data, Heap, Pointer},
    tracer::{FiberTracer, Tracer},
};

pub fn collect_symbols_in_heap(heap: &Heap) -> Vec<String> {
    heap.all_objects()
        .filter_map(|object| {
            if let Data::Symbol(symbol) = &object.data {
                Some(symbol.value.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[derive(Default)]
pub struct FuzzablesFinder {
    pub fuzzables: FxHashMap<Id, Pointer>,
}
#[derive(Default)]
pub struct FiberFuzzablesFinder {
    fuzzables: FxHashMap<Id, Pointer>,
}

impl Tracer for FuzzablesFinder {
    type ForFiber = FiberFuzzablesFinder;

    fn fiber_created(&mut self, _: FiberId) {}
    fn fiber_done(&mut self, _: FiberId) {}
    fn fiber_panicked(&mut self, _: FiberId, _: Option<FiberId>) {}
    fn fiber_canceled(&mut self, _: FiberId) {}
    fn fiber_execution_started(&mut self, _: FiberId) {}
    fn fiber_execution_ended(&mut self, _: FiberId) {}
    fn channel_created(&mut self, _: candy_vm::channel::ChannelId) {}

    fn tracer_for_fiber(&mut self, _: FiberId) -> Self::ForFiber {
        FiberFuzzablesFinder::default()
    }
    fn integrate_fiber_tracer(&mut self, tracer: Self::ForFiber, from: &Heap, to: &mut Heap) {
        let mapping = from.clone_to_other_heap(to);
        for (id, closure) in tracer.fuzzables {
            self.fuzzables.insert(id, mapping[&closure]);
        }
    }
}

impl FiberTracer for FiberFuzzablesFinder {
    fn value_evaluated(&mut self, _: Pointer, _: Pointer, _: &mut Heap) {}
    fn call_started(&mut self, _: Pointer, _: Pointer, _: Vec<Pointer>, _: Pointer, _: &mut Heap) {}
    fn call_ended(&mut self, _: Pointer, _: &mut Heap) {}

    fn found_fuzzable_closure(&mut self, definition: Pointer, closure: Pointer, heap: &mut Heap) {
        let definition = heap.get_hir_id(definition);
        heap.dup(closure);
        self.fuzzables.insert(definition, closure);
    }
}

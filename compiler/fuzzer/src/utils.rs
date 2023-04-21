use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Closure, Heap, Symbol},
    tracer::{FiberEvent, Tracer, VmEvent},
};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn collect_symbols_in_heap(heap: &Heap) -> FxHashSet<String> {
    heap.all_objects()
        .iter()
        .filter_map(|object| Symbol::try_from(*object).ok().map(|it| it.to_string()))
        .collect()
}

#[derive(Default)]
pub struct FuzzablesFinder {
    pub fuzzables: FxHashMap<Id, Closure>,
    pub heap: Heap,
}
impl Tracer for FuzzablesFinder {
    fn add(&mut self, event: VmEvent) {
        let VmEvent::InFiber { event, .. } = event else { return; };
        let FiberEvent::FoundFuzzableClosure { definition, closure, .. } = event else { return; };

        let closure = closure.clone_to_heap(&mut self.heap);
        self.fuzzables
            .insert(definition.get().to_owned(), closure.try_into().unwrap());
    }
}

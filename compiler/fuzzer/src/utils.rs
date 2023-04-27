use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Closure, Heap, Tag, Text},
    tracer::{FiberEvent, Tracer, VmEvent},
};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn collect_symbols_in_heap(heap: &Heap) -> FxHashSet<Text> {
    heap.iter()
        .filter_map(|object| Tag::try_from(object).ok().map(|it| it.symbol()))
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

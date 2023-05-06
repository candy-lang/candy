use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Function, Heap, Tag, Text},
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
    pub fuzzables: FxHashMap<Id, Function>,
    pub heap: Heap,
}
impl Tracer for FuzzablesFinder {
    fn add(&mut self, event: VmEvent) {
        let VmEvent::InFiber { event, .. } = event else { return; };
        let FiberEvent::FoundFuzzableFunction { definition, function, .. } = event else { return; };

        let function = function.clone_to_heap(&mut self.heap);
        self.fuzzables
            .insert(definition.get().to_owned(), function.try_into().unwrap());
    }
}

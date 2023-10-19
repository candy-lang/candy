use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Function, Heap, Tag, Text},
    tracer::Tracer,
};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn collect_symbols_in_heap(heap: &Heap) -> FxHashSet<Text> {
    heap.iter()
        .filter_map(|object| Tag::try_from(object).ok().map(|it| it.symbol()))
        .chain(heap.default_symbols().all_symbols())
        .collect()
}

#[derive(Default)]
pub struct FuzzablesFinder {
    pub fuzzables: FxHashMap<Id, Function>,
}
impl Tracer for FuzzablesFinder {
    fn found_fuzzable_function(
        &mut self,
        _heap: &mut Heap,
        definition: candy_vm::heap::HirId,
        function: Function,
    ) {
        function.dup();
        self.fuzzables.insert(definition.get().clone(), function);
    }
}

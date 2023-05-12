use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::{Function, Heap, HirId, Tag, Text},
    tracer::{FiberEnded, FiberTracer, Tracer},
};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn collect_symbols_in_heap<'h>(heap: &Heap<'h>) -> FxHashSet<Text<'h>> {
    heap.iter()
        .filter_map(|object| Tag::try_from(object).ok().map(|it| it.symbol()))
        .collect()
}

#[derive(Default)]
pub struct FuzzablesFinder<'h> {
    fuzzables: Option<FxHashMap<Id, Function<'h>>>,
}
impl<'h> FuzzablesFinder<'h> {
    pub fn fuzzables(&self) -> Option<&FxHashMap<Id, Function<'h>>> {
        self.fuzzables.as_ref()
    }
    pub fn into_fuzzables(self) -> FxHashMap<Id, Function<'h>> {
        self.fuzzables.expect("VM didn't finish execution yet.")
    }
}
impl<'h> Tracer<'h> for FuzzablesFinder<'h> {
    type ForFiber = FiberFuzzablesFinder<'h>;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberFuzzablesFinder::default()
    }
    fn root_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self::ForFiber>) {
        assert!(self.fuzzables.is_none());
        self.fuzzables = Some(ended.tracer.fuzzables);
    }
}

#[derive(Default)]
pub struct FiberFuzzablesFinder<'h> {
    fuzzables: FxHashMap<Id, Function<'h>>,
}
impl<'h> FiberTracer<'h> for FiberFuzzablesFinder<'h> {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberFuzzablesFinder::default()
    }
    fn child_fiber_ended<'a>(&mut self, ended: FiberEnded<'a, 'h, Self>) {
        self.fuzzables.extend(ended.tracer.fuzzables)
    }

    fn found_fuzzable_function(
        &mut self,
        _heap: &mut Heap<'h>,
        definition: HirId<'h>,
        function: Function<'h>,
    ) {
        function.dup();
        self.fuzzables.insert(definition.get().to_owned(), function);
    }

    fn dup_all_stored_objects(&self, _heap: &mut Heap) {
        for function in self.fuzzables.values() {
            function.dup();
        }
    }
}

use candy_frontend::hir::Id;
use candy_vm::{
    fiber::FiberId,
    heap::{Function, Heap, Tag, Text},
    tracer::{FiberEnded, FiberTracer, Tracer},
};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn collect_symbols_in_heap(heap: &Heap) -> FxHashSet<Text> {
    heap.iter()
        .filter_map(|object| Tag::try_from(object).ok().map(|it| it.symbol()))
        .collect()
}

#[derive(Default)]
pub struct FuzzablesFinder {
    fuzzables: Option<FxHashMap<Id, Function>>,
}
impl FuzzablesFinder {
    pub fn fuzzables(&self) -> Option<&FxHashMap<Id, Function>> {
        self.fuzzables.as_ref()
    }
    pub fn into_fuzzables(self) -> FxHashMap<Id, Function> {
        self.fuzzables.expect("VM didn't finish execution yet.")
    }
}
impl Tracer for FuzzablesFinder {
    type ForFiber = FiberFuzzablesFinder;

    fn root_fiber_created(&mut self) -> Self::ForFiber {
        FiberFuzzablesFinder::default()
    }
    fn root_fiber_ended(&mut self, ended: FiberEnded<Self::ForFiber>) {
        assert!(self.fuzzables.is_none());
        self.fuzzables = Some(ended.tracer.fuzzables);
    }
}

#[derive(Default)]
pub struct FiberFuzzablesFinder {
    fuzzables: FxHashMap<Id, Function>,
}
impl FiberTracer for FiberFuzzablesFinder {
    fn child_fiber_created(&mut self, _child: FiberId) -> Self {
        FiberFuzzablesFinder::default()
    }
    fn child_fiber_ended(&mut self, ended: FiberEnded<Self>) {
        self.fuzzables.extend(ended.tracer.fuzzables)
    }

    fn found_fuzzable_function(
        &mut self,
        _heap: &mut Heap,
        definition: candy_vm::heap::HirId,
        function: Function,
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

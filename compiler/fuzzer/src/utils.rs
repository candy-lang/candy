use candy_frontend::hir::Id;
use candy_vm::{
    heap::{Function, Heap},
    tracer::Tracer,
};
use rustc_hash::FxHashMap;

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

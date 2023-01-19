use rustc_hash::FxHashMap;

use crate::{
    compiler::hir::Id,
    vm::{
        tracer::{FiberEvent, Tracer, VmEvent},
        FiberId, Heap, Pointer,
    },
};

#[derive(Default)]
pub struct FuzzablesFinder {
    pub fuzzables: FxHashMap<Id, Pointer>,
    pub heap: Heap,
    transferred_objects: FxHashMap<FiberId, FxHashMap<Pointer, Pointer>>,
}
impl Tracer for FuzzablesFinder {
    fn add(&mut self, event: VmEvent) {
        let VmEvent::InFiber { fiber, event } = event else { return; };
        let FiberEvent::FoundFuzzableClosure { definition, closure, heap } = event else { return; };

        let definition = heap.get_hir_id(definition);
        let address_map = self
            .transferred_objects
            .entry(fiber)
            .or_insert_with(FxHashMap::default);
        let closure = heap.clone_single_to_other_heap_with_existing_mapping(
            &mut self.heap,
            closure,
            address_map,
        );
        self.fuzzables.insert(definition, closure);
    }
}

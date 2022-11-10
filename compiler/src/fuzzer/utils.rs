use crate::{
    compiler::hir::Id,
    vm::{
        tracer::{FiberEvent, Tracer, VmEvent},
        FiberId, Heap, Pointer,
    },
};
use std::collections::HashMap;

#[derive(Default)]
pub struct FuzzablesFinder {
    pub fuzzables: Vec<(Id, Pointer)>,
    pub heap: Heap,
    transferred_objects: HashMap<FiberId, HashMap<Pointer, Pointer>>,
}
impl Tracer for FuzzablesFinder {
    fn add(&mut self, event: VmEvent) {
        let VmEvent::InFiber { fiber, event } = event else { return; };
        let FiberEvent::FoundFuzzableClosure { definition, closure, heap } = event else { return; };

        let definition = self.heap.get_hir_id(definition);
        let address_map = self
            .transferred_objects
            .entry(fiber)
            .or_insert_with(HashMap::new);
        let address = heap.clone_single_to_other_heap_with_existing_mapping(
            &mut self.heap,
            closure,
            address_map,
        );
        self.fuzzables.push((definition, address));
    }
}

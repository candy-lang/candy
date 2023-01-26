use std::fmt;

use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::{
    compiler::hir::Id,
    vm::{
        tracer::{FiberEvent, Tracer, VmEvent},
        Data, FiberId, Heap, Pointer,
    },
};

#[derive(Clone)]
pub struct Input {
    pub heap: Heap,
    pub arguments: Vec<Pointer>,
}
impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.arguments
                .iter()
                .map(|arg| arg.format(&self.heap))
                .join(" "),
        )
    }
}
impl Input {
    pub fn clone_to_other_heap(&self, other: &mut Heap) -> Vec<Pointer> {
        self.heap
            .clone_multiple_to_other_heap_with_existing_mapping(
                other,
                &self.arguments,
                &mut FxHashMap::default(),
            )
    }
}

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
    pub heap: Heap,
    transferred_objects: FxHashMap<FiberId, FxHashMap<Pointer, Pointer>>,
}
impl Tracer for FuzzablesFinder {
    fn add(&mut self, event: VmEvent) {
        let VmEvent::InFiber { fiber, event } = event else { return; };
        let FiberEvent::FoundFuzzableClosure { definition, closure, heap } = event else { return; };
        assert!(matches!(heap.get(closure).data, Data::Closure(_)));

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
        assert!(matches!(self.heap.get(closure).data, Data::Closure(_)));
        self.fuzzables.insert(definition, closure);
    }
}

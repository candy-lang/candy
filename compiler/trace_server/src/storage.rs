use candy_frontend::id::{CountableId, IdGenerator};
use candy_vm::heap::{Heap, Pointer};
use rustc_hash::FxHashMap;
use std::fmt::Debug;

use crate::trace::{Trace, TraceId};

pub struct TraceStorage {
    pub heap: Heap,
    when_to_deduplicate: usize,
    traces: FxHashMap<TraceId, Trace>,
    id_generator: IdGenerator<TraceId>,
}
impl TraceStorage {
    pub fn new(heap: Heap) -> Self {
        Self {
            heap,
            when_to_deduplicate: 100,
            traces: FxHashMap::default(),
            id_generator: IdGenerator::default(),
        }
    }

    pub fn create(&mut self, trace: Trace) -> TraceId {
        let id = self.id_generator.generate();
        self.traces.insert(id, trace);
        id
    }

    pub fn get(&self, id: TraceId) -> &Trace {
        self.traces.get(&id).unwrap()
    }
    pub fn get_mut(&mut self, id: TraceId) -> &mut Trace {
        self.traces.get_mut(&id).unwrap()
    }

    pub fn import_from_heap(&mut self, heap: &Heap, address: Pointer) -> Pointer {
        heap.clone_single_to_other_heap(&mut self.heap, address)
    }

    pub fn maybe_deduplicate(&mut self) {
        if self.heap.number_of_objects() > self.when_to_deduplicate {
            let pointer_map = self.heap.deduplicate();
            for trace in self.traces.values_mut() {
                trace.change_pointers(&pointer_map);
            }
            self.when_to_deduplicate = (self.when_to_deduplicate as f64 * 1.1) as usize;
        }
    }
}

impl Debug for TraceStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.get(TraceId::from_usize(0)).format(&self))
    }
}

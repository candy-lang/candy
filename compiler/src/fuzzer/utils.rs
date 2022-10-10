use crate::{
    compiler::hir::Id,
    module::Module,
    vm::{
        tracer::{InFiberTracer, Tracer},
        ChannelId, FiberId, Heap, Pointer,
    },
};
use std::collections::HashMap;

pub struct FuzzablesFinder {
    pub fuzzables: Vec<(Id, Pointer)>,
    pub heap: Heap,
    transferred_objects: HashMap<FiberId, HashMap<Pointer, Pointer>>,
}
impl FuzzablesFinder {
    pub fn new() -> Self {
        Self {
            fuzzables: vec![],
            heap: Heap::default(),
            transferred_objects: HashMap::new(),
        }
    }
}
impl Tracer for FuzzablesFinder {
    fn fiber_created(&mut self, fiber: FiberId) {}
    fn fiber_done(&mut self, fiber: FiberId) {}
    fn fiber_panicked(&mut self, fiber: FiberId, panicked_child: Option<FiberId>) {}
    fn fiber_canceled(&mut self, fiber: FiberId) {}
    fn fiber_execution_started(&mut self, fiber: FiberId) {}
    fn fiber_execution_ended(&mut self, fiber: FiberId) {}
    fn channel_created(&mut self, channel: ChannelId) {}
    fn sent_to_channel(&mut self, value: Pointer, from: FiberId, to: ChannelId) {}
    fn received_from_channel(&mut self, value: Pointer, from: ChannelId, to: FiberId) {}

    fn in_fiber_tracer<'a>(&'a mut self, fiber: FiberId) -> Box<dyn InFiberTracer<'a> + 'a>
    where
        Self: 'a,
    {
        Box::new(InFiberFuzzablesFinder {
            tracer: self,
            fiber,
        })
    }
}
pub struct InFiberFuzzablesFinder<'a> {
    tracer: &'a mut FuzzablesFinder,
    fiber: FiberId,
}
impl<'a> InFiberTracer<'a> for InFiberFuzzablesFinder<'a> {
    fn module_started(&mut self, _module: Module) {}
    fn module_ended(&mut self, _heap: &Heap, _export_map: Pointer) {}
    fn value_evaluated(&mut self, _heap: &Heap, _id: Id, _value: Pointer) {}
    fn call_started(&mut self, _heap: &Heap, _id: Id, _closure: Pointer, _args: Vec<Pointer>) {}
    fn call_ended(&mut self, _heap: &Heap, _return_value: Pointer) {}
    fn needs_started(&mut self, _heap: &Heap, _id: Id, _condition: Pointer, _reason: Pointer) {}
    fn needs_ended(&mut self) {}

    fn found_fuzzable_closure(&mut self, heap: &Heap, id: Id, closure: Pointer) {
        let address_map = self
            .tracer
            .transferred_objects
            .entry(self.fiber)
            .or_insert_with(HashMap::new);
        let address = heap.clone_single_to_other_heap_with_existing_mapping(
            &mut self.tracer.heap,
            closure,
            address_map,
        );
        self.tracer.fuzzables.push((id, address));
    }
}

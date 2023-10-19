use self::{stack_trace::StackFrameKey, utils::IdMapping, variable::VariablesKey};
use super::DebugVm;
use candy_vm::heap::Heap;

mod memory;
mod scope;
mod stack_trace;
mod utils;
mod variable;

pub struct PausedState {
    pub vm: Option<PausedVm>, // only `None` during state transitions
    stack_frame_ids: IdMapping<StackFrameKey>,
    variables_ids: IdMapping<VariablesKey>,
}
impl PausedState {
    pub fn new(heap: Heap, vm: DebugVm) -> Self {
        Self {
            vm: Some(PausedVm::new(heap, vm)),
            stack_frame_ids: IdMapping::default(),
            variables_ids: IdMapping::default(),
        }
    }

    #[must_use]
    pub fn heap_ref(&self) -> &Heap {
        &self.vm.as_ref().unwrap().heap
    }
    #[must_use]
    pub fn vm_ref(&self) -> &DebugVm {
        &self.vm.as_ref().unwrap().vm
    }
}
pub struct PausedVm {
    pub heap: Heap,
    pub vm: DebugVm,
}
impl PausedVm {
    pub fn new(heap: Heap, vm: DebugVm) -> Self {
        Self { heap, vm }
    }
}

use self::{stack_trace::StackFrameKey, utils::IdMapping, variable::VariablesKey};
use super::vm_state::VmState;

mod memory;
mod scope;
mod stack_trace;
mod utils;
mod variable;

pub struct PausedState {
    pub vm_state: VmState,
    stack_frame_ids: IdMapping<StackFrameKey>,
    variables_ids: IdMapping<VariablesKey>,
}
impl PausedState {
    pub fn new(vm_state: VmState) -> Self {
        Self {
            vm_state,
            stack_frame_ids: IdMapping::default(),
            variables_ids: IdMapping::default(),
        }
    }
}

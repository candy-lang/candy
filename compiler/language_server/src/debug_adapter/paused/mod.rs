use self::{stack_trace::StackFrameKey, utils::IdMapping, variable::VariablesKey};
use super::session::VmState;

mod scope;
mod stack_trace;
mod utils;
mod variable;

pub struct PausedState<'c: 'h, 'h> {
    pub vm_state: VmState<'c, 'h>,
    stack_frame_ids: IdMapping<StackFrameKey>,
    variables_ids: IdMapping<VariablesKey<'h>>,
}
impl<'c: 'h, 'h> PausedState<'c, 'h> {
    pub fn new(vm_state: VmState<'c, 'h>) -> Self {
        Self {
            vm_state,
            stack_frame_ids: IdMapping::default(),
            variables_ids: IdMapping::default(),
        }
    }
}

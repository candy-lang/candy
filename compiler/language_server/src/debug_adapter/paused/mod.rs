use self::{stack_trace::StackFrameKey, utils::IdMapping, variable::VariablesKey};
use super::DebugVm;

mod memory;
mod scope;
mod stack_trace;
mod utils;
mod variable;

pub struct PausedState {
    pub vm: Option<DebugVm>, // only `None` during state transitions
    stack_frame_ids: IdMapping<StackFrameKey>,
    variables_ids: IdMapping<VariablesKey>,
}
impl PausedState {
    pub fn new(vm: DebugVm) -> Self {
        Self {
            vm: Some(vm),
            stack_frame_ids: IdMapping::default(),
            variables_ids: IdMapping::default(),
        }
    }

    // pub fn vm() -> &DebugVm {
    //     self.vm.as_ref().unwrap()
    // }
}

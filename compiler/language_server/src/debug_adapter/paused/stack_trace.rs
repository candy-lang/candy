use super::PausedState;
use crate::debug_adapter::{tracer::DebugTracer, utils::FiberIdThreadIdConversion};
use candy_frontend::utils::AdjustCasingOfFirstLetter;
use candy_vm::{fiber::FiberId, heap::Data, tracer::stack_trace::Call};
use dap::{
    requests::StackTraceArguments,
    responses::StackTraceResponse,
    types::{StackFrame, StackFramePresentationhint},
};
use std::hash::Hash;

impl PausedState {
    pub fn stack_trace(
        &mut self,
        args: StackTraceArguments,
    ) -> Result<StackTraceResponse, &'static str> {
        let fiber_id = FiberId::from_thread_id(args.thread_id);
        let fiber_state = self
            .vm_state
            .tracer
            .fibers
            .get(&fiber_id)
            .ok_or("fiber-not-found")?;

        let start_frame = args.start_frame.map(|it| it as usize).unwrap_or_default();
        let mut call_stack = &fiber_state.call_stack[start_frame..];
        if let Some(levels) = args.levels {
            let levels = levels as usize;
            if levels < call_stack.len() {
                call_stack = &call_stack[..levels];
            }
        }

        let stack_frames = call_stack
            .iter()
            .enumerate()
            .map(|(index, it)| {
                // TODO: format arguments
                let name = match Data::from(it.callee) {
                    // TODO: resolve function name
                    Data::Closure(closure) => format!("Closure at {:p}", closure.address()),
                    Data::Builtin(builtin) => format!(
                        "✨.{}",
                        format!("{:?}", builtin.get()).lowercase_first_letter(),
                    ),
                    Data::Tag(tag) => tag.symbol().get().to_owned(),
                    it => panic!("Unexpected callee: {it}"),
                };
                StackFrame {
                    id: self
                        .stack_frame_ids
                        .key_to_id(StackFrameKey { fiber_id, index }),
                    name,
                    source: None,
                    line: 1,
                    column: 1,
                    end_line: None,
                    end_column: None,
                    can_restart: Some(false),
                    instruction_pointer_reference: None,
                    module_id: None,
                    presentation_hint: Some(StackFramePresentationhint::Normal),
                }
            })
            .collect();
        let total_frames = fiber_state.call_stack.len() as i64;
        Ok(StackTraceResponse {
            stack_frames,
            total_frames: Some(total_frames),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StackFrameKey {
    pub fiber_id: FiberId,
    index: usize,
}
impl StackFrameKey {
    pub fn get<'a>(&self, tracer: &'a DebugTracer) -> &'a Call {
        &tracer.fibers.get(&self.fiber_id).unwrap().call_stack[self.index]
    }
}

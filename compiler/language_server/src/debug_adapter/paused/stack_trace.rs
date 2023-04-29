use super::PausedState;
use crate::debug_adapter::{
    tracer::{DebugTracer, FiberState, StackFrame},
    utils::FiberIdThreadIdConversion,
};
use candy_frontend::{hir::Id, utils::AdjustCasingOfFirstLetter};
use candy_vm::{
    fiber::FiberId,
    heap::{Data, InlineObject},
};
use dap::{
    self, requests::StackTraceArguments, responses::StackTraceResponse,
    types::StackFramePresentationhint,
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
        let levels = args
            .levels
            .and_then(|it| if it == 0 { None } else { Some(it as usize) })
            .unwrap_or(usize::MAX);
        let call_stack = &fiber_state.call_stack[..fiber_state.call_stack.len() - start_frame];

        let mut stack_frames = Vec::with_capacity((1 + call_stack.len()).min(levels));
        stack_frames.extend(call_stack.iter().enumerate().rev().skip(start_frame).map(
            |(index, it)| {
                // TODO: format arguments
                let name = match Data::from(it.call.callee) {
                    // TODO: resolve function name
                    Data::Closure(closure) => format!("Closure at {:p}", closure.address()),
                    Data::Builtin(builtin) => format!(
                        "✨.{}",
                        format!("{:?}", builtin.get()).lowercase_first_letter(),
                    ),
                    Data::Tag(tag) => tag.symbol().get().to_owned(),
                    it => panic!("Unexpected callee: {it}"),
                };
                dap::types::StackFrame {
                    id: self.stack_frame_ids.key_to_id(StackFrameKey {
                        fiber_id,
                        index: index + 1,
                    }),
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
            },
        ));

        if stack_frames.len() < levels {
            stack_frames.push(dap::types::StackFrame {
                id: self
                    .stack_frame_ids
                    .key_to_id(StackFrameKey { fiber_id, index: 0 }),
                name: "Spawn".to_string(),
                source: None,
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
                can_restart: Some(false),
                instruction_pointer_reference: None,
                module_id: None,
                presentation_hint: Some(StackFramePresentationhint::Normal),
            });
        }

        Ok(StackTraceResponse {
            stack_frames,
            total_frames: Some((fiber_state.call_stack.len() + 1) as i64),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StackFrameKey {
    pub fiber_id: FiberId,

    /// `0` represents the fiber spawn for which we don't have a stack frame.
    index: usize,
}
impl StackFrameKey {
    pub fn get<'a>(&self, tracer: &'a DebugTracer) -> Option<&'a StackFrame> {
        if self.index == 0 {
            return None;
        }

        Some(&self.get_fiber_state(tracer).call_stack[self.index - 1])
    }
    pub fn get_locals<'a>(&self, tracer: &'a DebugTracer) -> &'a Vec<(Id, InlineObject)> {
        let fiber_state = self.get_fiber_state(tracer);
        if self.index == 0 {
            &fiber_state.root_locals
        } else {
            &fiber_state.call_stack[self.index - 1].locals
        }
    }
    fn get_fiber_state<'a>(&self, tracer: &'a DebugTracer) -> &'a FiberState {
        tracer.fibers.get(&self.fiber_id).unwrap()
    }
}

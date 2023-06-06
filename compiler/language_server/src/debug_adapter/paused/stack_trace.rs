use super::{utils::FiberIdExtension, PausedState};
use crate::debug_adapter::{
    tracer::{DebugTracer, StackFrame},
    utils::FiberIdThreadIdConversion,
};
use candy_frontend::{hir::Id, utils::AdjustCasingOfFirstLetter};
use candy_vm::{
    fiber::FiberId,
    heap::{Data, InlineObject},
    lir::Lir,
    vm::Vm,
};
use dap::{
    self, requests::StackTraceArguments, responses::StackTraceResponse,
    types::StackFramePresentationhint,
};
use std::{borrow::Borrow, hash::Hash};

impl PausedState {
    pub fn stack_trace(
        &mut self,
        args: StackTraceArguments,
    ) -> Result<StackTraceResponse, &'static str> {
        let fiber_id = FiberId::from_thread_id(args.thread_id);
        let fiber = self
            .vm_state
            .vm
            .fiber(fiber_id)
            .ok_or("fiber-not-found")?
            .fiber_ref();
        let fiber_state = &fiber.tracer;

        let start_frame = args.start_frame.unwrap_or_default();
        let levels = args
            .levels
            .and_then(|it| if it == 0 { None } else { Some(it) })
            .unwrap_or(usize::MAX);
        let call_stack = &fiber_state.call_stack[..fiber_state.call_stack.len() - start_frame];

        let mut stack_frames = Vec::with_capacity((1 + call_stack.len()).min(levels));
        stack_frames.extend(call_stack.iter().enumerate().rev().skip(start_frame).map(
            |(index, it)| {
                // TODO: format arguments
                let name = match Data::from(it.call.callee) {
                    // TODO: resolve function name
                    Data::Function(function) => format!("Function at {:p}", function.address()),
                    Data::Builtin(builtin) => format!(
                        "✨.{}",
                        format!("{:?}", builtin.get()).lowercase_first_letter(),
                    ),
                    it => panic!("Unexpected callee: {it}"),
                };
                dap::types::StackFrame {
                    id: self
                        .stack_frame_ids
                        .key_to_id(StackFrameKey {
                            fiber_id,
                            index: index + 1,
                        })
                        .get(),
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
                    .key_to_id(StackFrameKey { fiber_id, index: 0 })
                    .get(),
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
            total_frames: Some(fiber_state.call_stack.len() + 1),
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
    pub fn get<'a, L: Borrow<Lir>>(&self, vm: &'a Vm<L, DebugTracer>) -> Option<&'a StackFrame> {
        if self.index == 0 {
            return None;
        }

        Some(&self.fiber_id.get(vm).tracer.call_stack[self.index - 1])
    }
    pub fn get_locals<'a, L: Borrow<Lir>>(
        &self,
        vm: &'a Vm<L, DebugTracer>,
    ) -> &'a Vec<(Id, InlineObject)> {
        let fiber_state = &self.fiber_id.get(vm).tracer;
        if self.index == 0 {
            &fiber_state.root_locals
        } else {
            &fiber_state.call_stack[self.index - 1].locals
        }
    }
}

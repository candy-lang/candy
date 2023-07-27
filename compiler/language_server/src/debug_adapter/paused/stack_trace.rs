use super::{utils::FiberIdExtension, PausedState};
use crate::{
    database::Database,
    debug_adapter::{
        session::StartAt1Config,
        tracer::{DebugTracer, StackFrame},
        utils::FiberIdThreadIdConversion,
    },
    utils::{module_to_url, LspPositionConversion},
};
use candy_frontend::{ast_to_hir::AstToHir, hir::Id, utils::AdjustCasingOfFirstLetter};
use candy_vm::{
    fiber::FiberId,
    heap::{Data, DisplayWithSymbolTable, InlineObject},
    lir::Lir,
    vm::Vm,
};
use dap::{
    self,
    requests::StackTraceArguments,
    responses::StackTraceResponse,
    types::{PresentationHint, Source, StackFramePresentationhint},
};
use std::{borrow::Borrow, hash::Hash};

impl PausedState {
    pub fn stack_trace(
        &mut self,
        db: &Database,
        start_at_1_config: StartAt1Config,
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
        let total_frames = fiber_state.call_stack.len() + 1;

        let mut stack_frames = Vec::with_capacity((1 + call_stack.len()).min(levels));
        stack_frames.extend(call_stack.iter().enumerate().rev().skip(start_frame).map(
            |(index, frame)| {
                let id = self
                    .stack_frame_ids
                    .key_to_id(StackFrameKey {
                        fiber_id,
                        index: index + 1,
                    })
                    .get();
                Self::stack_frame(db, start_at_1_config, id, frame, self.vm_state.vm.lir())
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
            total_frames: Some(total_frames),
        })
    }

    fn stack_frame(
        db: &Database,
        start_at_1_config: StartAt1Config,
        id: usize,
        frame: &StackFrame,
        lir: &Lir,
    ) -> dap::types::StackFrame {
        let (name, source, range) = match Data::from(frame.call.callee) {
            Data::Function(function) => {
                let functions = lir.functions_behind(function.body());
                assert_eq!(functions.len(), 1);
                let function = functions.iter().next().unwrap();

                let source = Source {
                    name: Some(function.module.to_string()),
                    path: Some(
                        module_to_url(&function.module, &db.packages_path)
                            .unwrap()
                            .to_string(),
                    ),
                    source_reference: None,
                    presentation_hint: if lir.module.package == function.module.package {
                        PresentationHint::Emphasize
                    } else {
                        PresentationHint::Normal
                    },
                    origin: None,
                    sources: None,
                    adapter_data: None,
                    checksums: None,
                };
                let range = db.hir_id_to_span(function.to_owned()).unwrap();
                let range = db.range_to_lsp_range(function.module.to_owned(), range);
                let range = start_at_1_config.range_to_dap(range);
                (function.function_name(), Some(source), Some(range))
            }
            Data::Builtin(builtin) => {
                let name = format!(
                    "✨.{}",
                    format!("{:?}", builtin.get()).lowercase_first_letter(),
                );
                (name, None, None)
            }
            it => panic!(
                "Unexpected callee: {}",
                DisplayWithSymbolTable::to_string(&it, &lir.symbol_table),
            ),
        };
        dap::types::StackFrame {
            id,
            name,
            source,
            line: range.map(|it| it.start.line as usize).unwrap_or(1),
            column: range.map(|it| it.start.character as usize).unwrap_or(1),
            end_line: range.map(|it| it.end.line as usize),
            end_column: range.map(|it| it.end.character as usize),
            can_restart: Some(false),
            instruction_pointer_reference: None,
            module_id: None,
            presentation_hint: Some(StackFramePresentationhint::Normal),
        }
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

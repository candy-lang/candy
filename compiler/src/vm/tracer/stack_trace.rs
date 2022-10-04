use super::{super::heap::Pointer, Event, FiberId, FullTracer, Heap, InFiberEvent};
use crate::{
    compiler::{ast_to_hir::AstToHir, cst::CstDb, hir::Id},
    database::Database,
    language_server::utils::LspPositionConversion,
    module::Module,
};
use itertools::Itertools;
use std::collections::HashMap;
use tracing::error;

// Stack traces are a reduced view of the tracing state that represent the stack
// trace at a given moment in time.

#[derive(Clone)]
pub enum StackEntry {
    Call {
        id: Id,
        closure: Pointer,
        args: Vec<Pointer>,
    },
    Needs {
        id: Id,
        condition: Pointer,
        reason: Pointer,
    },
    Module {
        module: Module,
    },
}

impl FullTracer {
    pub fn stack_traces(&self) -> HashMap<FiberId, Vec<StackEntry>> {
        let mut stacks: HashMap<FiberId, Vec<StackEntry>> = HashMap::new();
        for timed_event in &self.events {
            if let Event::InFiber { fiber, event } = &timed_event.event {
                let stack = stacks.entry(*fiber).or_default();
                match event {
                    InFiberEvent::CallStarted { id, closure, args } => {
                        stack.push(StackEntry::Call {
                            id: id.clone(),
                            closure: closure.clone(),
                            args: args.clone(),
                        });
                    }
                    InFiberEvent::CallEnded { .. } => {
                        stack.pop().unwrap();
                    }
                    InFiberEvent::ModuleStarted { module } => {
                        stack.push(StackEntry::Module {
                            module: module.clone(),
                        });
                    }
                    InFiberEvent::ModuleEnded { .. } => {
                        stack.pop().unwrap();
                    }
                    _ => {}
                }
            }
        }
        stacks
    }
    pub fn format_stack_traces(&self, db: &Database, heap: &Heap) -> String {
        let mut lines = vec![];

        for (fiber, stack) in self.stack_traces() {
            lines.push(format!("{fiber:?}:"));
            for entry in stack.iter().rev() {
                let (call_string, hir_id) = match entry {
                    StackEntry::Call { id, closure, args } => (
                        format!(
                            "{closure} {}",
                            args.iter().map(|arg| arg.format(heap)).join(" ")
                        ),
                        Some(id),
                    ),
                    StackEntry::Needs {
                        id,
                        condition,
                        reason,
                    } => (
                        format!("needs {} {}", condition.format(heap), reason.format(heap)),
                        Some(id),
                    ),
                    StackEntry::Module { module } => (format!("use {module}"), None),
                };
                let caller_location_string = {
                    let (hir_id, ast_id, cst_id, span) = if let Some(hir_id) = hir_id {
                        let module = hir_id.module.clone();
                        let ast_id = db.hir_to_ast_id(hir_id.clone());
                        let cst_id = db.hir_to_cst_id(hir_id.clone());
                        let cst = cst_id.map(|id| db.find_cst(module.clone(), id));
                        let span = cst.map(|cst| {
                            (
                                db.offset_to_lsp(module.clone(), cst.span.start),
                                db.offset_to_lsp(module.clone(), cst.span.end),
                            )
                        });
                        (Some(hir_id), ast_id, cst_id, span)
                    } else {
                        (None, None, None, None)
                    };
                    format!(
                        "{}, {}, {}, {}",
                        hir_id
                            .map(|id| format!("{id}"))
                            .unwrap_or_else(|| "<no hir>".to_string()),
                        ast_id
                            .map(|id| format!("{id}"))
                            .unwrap_or_else(|| "<no ast>".to_string()),
                        cst_id
                            .map(|id| format!("{id}"))
                            .unwrap_or_else(|| "<no cst>".to_string()),
                        span.map(|((start_line, start_col), (end_line, end_col))| format!(
                            "{}:{} – {}:{}",
                            start_line, start_col, end_line, end_col
                        ))
                        .unwrap_or_else(|| "<no location>".to_string())
                    )
                };
                lines.push(format!("{caller_location_string:90} {call_string}"));
            }
        }
        lines.join("\n")
    }
    pub fn dump_stack_traces(&self, db: &Database, heap: &Heap) {
        for line in self.format_stack_traces(db, heap).lines() {
            error!("{}", line);
        }
    }
}

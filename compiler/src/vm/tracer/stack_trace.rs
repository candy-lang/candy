use super::{
    super::heap::Pointer,
    full::{FullTracer, StoredFiberEvent, StoredVmEvent},
    FiberId,
};
use crate::{
    compiler::{
        ast_to_hir::AstToHir,
        cst::{Cst, CstDb, CstKind},
        hir::Id,
    },
    database::Database,
    language_server::utils::LspPositionConversion,
    module::Module,
};
use itertools::Itertools;
use pad::PadStr;
use std::collections::HashMap;

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
            let StoredVmEvent::InFiber { fiber, event } = &timed_event.event else { continue; };
            let stack = stacks.entry(*fiber).or_default();
            match event {
                StoredFiberEvent::ModuleStarted { module } => {
                    stack.push(StackEntry::Module {
                        module: module.clone(),
                    });
                }
                StoredFiberEvent::ModuleEnded { .. } => {
                    assert!(matches!(stack.pop().unwrap(), StackEntry::Module { .. }));
                }
                StoredFiberEvent::CallStarted { id, closure, args } => {
                    stack.push(StackEntry::Call {
                        id: id.clone(),
                        closure: *closure,
                        args: args.clone(),
                    });
                }
                StoredFiberEvent::CallEnded { .. } => {
                    assert!(matches!(stack.pop().unwrap(), StackEntry::Call { .. }));
                }
                StoredFiberEvent::NeedsStarted {
                    id,
                    condition,
                    reason,
                } => {
                    stack.push(StackEntry::Needs {
                        id: id.clone(),
                        condition: *condition,
                        reason: *reason,
                    });
                }
                StoredFiberEvent::NeedsEnded => {
                    assert!(matches!(stack.pop().unwrap(), StackEntry::Needs { .. }));
                }
                _ => {}
            }
        }
        stacks
    }
    pub fn format_stack_trace(&self, db: &Database, stack: &[StackEntry]) -> String {
        let mut caller_locations_and_calls = vec![];

        for entry in stack.iter().rev() {
            let hir_id = match entry {
                StackEntry::Call { id, .. } => Some(id),
                StackEntry::Needs { id, .. } => Some(id),
                StackEntry::Module { .. } => None,
            };
            let (cst_id, span) = if let Some(hir_id) = hir_id {
                let module = hir_id.module.clone();
                let cst_id = db.hir_to_cst_id(hir_id.clone());
                let cst = cst_id.map(|id| db.find_cst(module.clone(), id));
                let span = cst.map(|cst| {
                    (
                        db.offset_to_lsp(module.clone(), cst.span.start),
                        db.offset_to_lsp(module.clone(), cst.span.end),
                    )
                });
                (cst_id, span)
            } else {
                (None, None)
            };
            let caller_location_string = format!(
                "{} {}",
                hir_id
                    .map(|id| format!("{id}"))
                    .unwrap_or_else(|| "<no hir>".to_string()),
                span.map(|((start_line, start_col), (end_line, end_col))| format!(
                    "{}:{} – {}:{}",
                    start_line, start_col, end_line, end_col
                ))
                .unwrap_or_else(|| "<no location>".to_string())
            );
            let call_string = match entry {
                StackEntry::Call { closure, args, .. } => format!(
                    "{} {}",
                    cst_id
                        .and_then(|id| {
                            let cst = db.find_cst(hir_id.unwrap().module.clone(), id);
                            match cst.kind {
                                CstKind::Call { receiver, .. } => receiver.extract_receiver_name(),
                                _ => None,
                            }
                        })
                        .unwrap_or_else(|| closure.format(&self.heap)),
                    args.iter().map(|arg| arg.format(&self.heap)).join(" ")
                ),
                StackEntry::Needs {
                    condition, reason, ..
                } => format!(
                    "needs {} {}",
                    condition.format(&self.heap),
                    reason.format(&self.heap),
                ),
                StackEntry::Module { module } => format!("module {module}"),
            };
            caller_locations_and_calls.push((caller_location_string, call_string));
        }

        let longest_location = caller_locations_and_calls
            .iter()
            .map(|(location, _)| location.len())
            .max()
            .unwrap();

        caller_locations_and_calls
            .into_iter()
            .map(|(location, call)| format!("{} {}", location.pad_to_width(longest_location), call))
            .join("\n")
    }
    /// When a VM panics, some child fiber might be responsible for that. This
    /// function returns a formatted stack trace spanning all fibers in the
    /// chain from the panicking root fiber until the concrete failing needs.
    pub fn format_panic_stack_trace_to_root_fiber(&self, db: &Database) -> String {
        let mut panicking_fiber_chain = vec![FiberId::root()];
        for timed_event in self.events.iter().rev() {
            if let StoredVmEvent::FiberPanicked {
                fiber,
                panicked_child,
            } = timed_event.event
            {
                if fiber == *panicking_fiber_chain.last().unwrap() {
                    match panicked_child {
                        Some(child) => panicking_fiber_chain.push(child),
                        None => break,
                    }
                }
            }
        }

        let stack_traces = self.stack_traces();
        panicking_fiber_chain
            .into_iter()
            .rev()
            .map(|fiber| self.format_stack_trace(db, &stack_traces[&fiber]))
            .join("\n(fiber boundary)\n")
    }
}

impl Cst {
    fn extract_receiver_name(&self) -> Option<String> {
        match &self.kind {
            CstKind::TrailingWhitespace { child, .. } => child.extract_receiver_name(),
            CstKind::Identifier(identifier) => Some(identifier.to_string()),
            CstKind::Parenthesized { inner, .. } => inner.extract_receiver_name(),
            CstKind::StructAccess { struct_, key, .. } => {
                let struct_string = struct_.extract_receiver_name()?;
                let key = key.extract_receiver_name()?;
                Some(format!("{struct_string}.{key}"))
            }
            _ => None,
        }
    }
}

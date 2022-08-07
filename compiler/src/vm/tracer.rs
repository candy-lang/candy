use super::{heap::Pointer, Heap};
use crate::{
    compiler::{ast_to_hir::AstToHir, cst::CstDb, hir::Id},
    database::Database,
    language_server::utils::LspPositionConversion,
    module::Module,
};
use itertools::Itertools;

#[derive(Default, Clone)]
pub struct Tracer {
    log: Vec<TraceEntry>,
    stack: Vec<TraceEntry>,
}
#[derive(Clone)]
pub enum TraceEntry {
    ValueEvaluated {
        id: Id,
        value: Pointer,
    },
    CallStarted {
        id: Id,
        closure: Pointer,
        args: Vec<Pointer>,
    },
    CallEnded {
        return_value: Pointer,
    },
    NeedsStarted {
        id: Id,
        condition: Pointer,
        reason: Pointer,
    },
    NeedsEnded,
    ModuleStarted {
        module: Module,
    },
    ModuleEnded {
        export_map: Pointer,
    },
}

impl Tracer {
    pub fn push(&mut self, entry: TraceEntry) {
        self.log.push(entry.clone());
        match entry {
            TraceEntry::CallStarted { .. } => {
                self.stack.push(entry);
            }
            TraceEntry::CallEnded { .. } => {
                self.stack.pop().unwrap();
            }
            TraceEntry::NeedsStarted { .. } => {
                self.stack.push(entry);
            }
            TraceEntry::NeedsEnded => {
                self.stack.pop().unwrap();
            }
            TraceEntry::ModuleStarted { .. } => {
                self.stack.push(entry);
            }
            TraceEntry::ModuleEnded { .. } => {
                self.stack.pop().unwrap();
            }
            _ => {}
        }
    }
    pub fn log(&self) -> &[TraceEntry] {
        &self.log
    }

    pub fn stack(&self) -> &[TraceEntry] {
        &self.stack
    }
    pub fn dump_stack_trace(&self, db: &Database, heap: &Heap) {
        for line in self.format_stack_trace(db, heap).lines() {
            log::error!("{}", line);
        }
    }
    pub fn format_stack_trace(&self, db: &Database, heap: &Heap) -> String {
        // TODO: Format values properly.
        self.stack
            .iter()
            .rev()
            .map(|entry| {
                let (call_string, hir_id) = match entry {
                    TraceEntry::CallStarted { id, closure, args } => (
                        format!(
                            "{closure} {}",
                            args.iter().map(|arg| arg.format(heap)).join(" ")
                        ),
                        Some(id),
                    ),
                    TraceEntry::NeedsStarted {
                        id,
                        condition,
                        reason,
                    } => (
                        format!("needs {} {}", condition.format(heap), reason.format(heap)),
                        Some(id),
                    ),
                    TraceEntry::ModuleStarted { module } => (format!("module {module}"), None),
                    _ => unreachable!(),
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
                format!("{caller_location_string:90} {call_string}")
            })
            .join("\n")
    }

    pub fn dump_call_tree(&self) -> String {
        // TODO: Format values properly.
        let actions = self
            .log
            .iter()
            .map(|entry| match entry {
                TraceEntry::ValueEvaluated { id, value } => Action::Stay(format!("{id} = {value}")),
                TraceEntry::CallStarted { id, closure, args } => Action::Start(format!(
                    "{id} {closure} {}",
                    args.iter().map(|arg| format!("{arg}")).join(" ")
                )),
                TraceEntry::CallEnded { return_value } => Action::End(format!(" = {return_value}")),
                TraceEntry::NeedsStarted {
                    id,
                    condition,
                    reason,
                } => Action::Start(format!("{id} needs {condition} {reason}")),
                TraceEntry::NeedsEnded => Action::End(" = Nothing".to_string()),
                TraceEntry::ModuleStarted { module } => Action::Start(format!("module {module}")),
                TraceEntry::ModuleEnded { export_map } => Action::End(format!("{export_map}")),
            })
            .collect_vec();

        let mut lines = vec![];
        let mut stack = vec![];
        let mut indentation = 0;

        for action in actions {
            let indent = "  ".repeat(indentation);
            match action {
                Action::Start(line) => {
                    stack.push(lines.len());
                    lines.push(format!("{indent}{line}"));
                    indentation += 1;
                }
                Action::End(addendum) => {
                    let start = stack.pop().unwrap();
                    lines[start].push_str(&addendum);
                    indentation -= 1;
                }
                Action::Stay(line) => {
                    lines.push(format!("{indent}{line}"));
                }
            }
        }

        lines.join("\n")
    }
}

enum Action {
    Start(String),
    End(String),
    Stay(String),
}

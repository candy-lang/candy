use super::value::Value;
use crate::{
    compiler::{ast_to_hir::AstToHir, cst::CstDb, cst_to_ast::CstToAst, hir::Id},
    database::Database,
    input::Input,
    language_server::utils::LspPositionConversion,
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
        value: Value,
    },
    CallStarted {
        id: Id,
        closure: Value,
        args: Vec<Value>,
    },
    CallEnded {
        return_value: Value,
    },
    NeedsStarted {
        id: Id,
        condition: Value,
        message: Value,
    },
    NeedsEnded,
    ModuleStarted {
        input: Input,
    },
    ModuleEnded {
        export_map: Value,
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

    pub fn dump_stack_trace(&self, db: &Database, input: Input) {
        for line in self.format_stack_trace(db, input).lines() {
            log::error!("{}", line);
        }
    }
    pub fn format_stack_trace(&self, db: &Database, input: Input) -> String {
        let (_, hir_to_ast_ids) = db.hir(input.clone()).unwrap();
        let (_, ast_to_cst_ids) = db.ast(input.clone()).unwrap();

        self.stack
            .iter()
            .rev()
            .map(|entry| {
                let (call_string, hir_id) = match entry {
                    TraceEntry::CallStarted { id, closure, args } => (
                        format!(
                            "{closure} {}",
                            args.iter().map(|arg| format!("{arg}")).join(" ")
                        ),
                        Some(id),
                    ),
                    TraceEntry::NeedsStarted {
                        id,
                        condition,
                        message,
                    } => (format!("needs {condition} {message}"), Some(id)),
                    TraceEntry::ModuleStarted { input } => (format!("module {input}"), None),
                    _ => unreachable!(),
                };
                let caller_location_string = {
                    let ast_id = hir_id
                        .and_then(|id| hir_to_ast_ids.get(&id))
                        .map(|id| id.clone());
                    let cst_id = ast_id
                        .as_ref()
                        .and_then(|id| ast_to_cst_ids.get(&id))
                        .map(|id| id.clone());
                    let cst = cst_id
                        .map(|id| db.find_cst(input.clone(), id))
                        .map(|id| id.clone());
                    let span = cst.map(|cst| {
                        (
                            db.offset_to_lsp(input.clone(), cst.span.start),
                            db.offset_to_lsp(input.clone(), cst.span.end),
                        )
                    });
                    format!(
                        "{}, {}, {}, {}",
                        hir_id
                            .map(|id| format!("{id}"))
                            .unwrap_or("<no hir>".to_string()),
                        ast_id
                            .map(|id| format!("{id}"))
                            .unwrap_or("<no ast>".to_string()),
                        cst_id
                            .map(|id| format!("{id}"))
                            .unwrap_or("<no cst>".to_string()),
                        span.map(|((start_line, start_col), (end_line, end_col))| format!(
                            "{}:{} – {}:{}",
                            start_line, start_col, end_line, end_col
                        ))
                        .unwrap_or("<no location>".to_string())
                    )
                };
                format!("{caller_location_string:90} {call_string}")
            })
            .join("\n")
    }

    pub fn dump_call_tree(&self) -> String {
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
                    message,
                } => Action::Start(format!("{id} needs {condition} {message}")),
                TraceEntry::NeedsEnded => Action::End(" = Nothing".to_string()),
                TraceEntry::ModuleStarted { input } => Action::Start(format!("module {input}")),
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

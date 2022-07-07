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
                        id,
                    ),
                    TraceEntry::NeedsStarted {
                        id,
                        condition,
                        message,
                    } => (format!("needs {condition} {message}"), id),
                    _ => unreachable!(),
                };
                let caller_location_string = {
                    let ast_id = hir_to_ast_ids.get(&hir_id).map(|id| id.clone());
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
                        "{hir_id}, {}, {}, {}",
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
        let mut calls = vec![];
        let mut stack = vec![];
        let mut indentation = 0;

        for entry in &self.log {
            match entry {
                TraceEntry::ValueEvaluated { .. } => {}
                TraceEntry::CallStarted { id, closure, args } => {
                    stack.push(calls.len());
                    calls.push(DumpableCall {
                        indentation,
                        id: id.clone(),
                        closure: closure.clone(),
                        args: args.clone(),
                        return_value: None,
                    });
                    indentation += 1;
                }
                TraceEntry::CallEnded { return_value } => {
                    let start = stack.pop().unwrap();
                    calls[start].return_value = Some(return_value.clone());
                    indentation -= 1;
                }
                TraceEntry::NeedsStarted {
                    id,
                    condition,
                    message,
                } => {
                    stack.push(calls.len());
                    calls.push(DumpableCall {
                        indentation,
                        id: id.clone(),
                        closure: Value::Symbol("Needs".to_string()),
                        args: vec![condition.clone(), message.clone()],
                        return_value: None,
                    });
                    indentation += 1;
                }
                TraceEntry::NeedsEnded => {
                    let start = stack.pop().unwrap();
                    calls[start].return_value = Some(Value::nothing());
                    indentation -= 1;
                }
            }
        }

        let mut dump = "".to_string();
        for call in calls {
            dump.push_str(&"  ".repeat(call.indentation));
            dump.push_str(&format!("{}", call.id));
            dump.push(' ');
            dump.push_str(&format!("{}", call.closure));
            for arg in call.args {
                dump.push(' ');
                dump.push_str(&format!("{arg}"));
            }
            if let Some(value) = call.return_value {
                dump.push_str(" = ");
                dump.push_str(&format!("{value}"));
            } else {
                dump.push_str(" (panicked)");
            }
            dump.push('\n');
        }
        dump
    }
}

struct DumpableCall {
    indentation: usize,
    id: Id,
    closure: Value,
    args: Vec<Value>,
    return_value: Option<Value>,
}

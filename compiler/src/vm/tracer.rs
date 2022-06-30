use super::value::Value;
use crate::compiler::hir::Id;

#[derive(Default, Clone)]
pub struct Tracer {
    pub log: Vec<TraceEntry>,
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
        self.log.push(entry);
    }
    pub fn correlate_and_dump(&self) -> String {
        let mut calls = vec![];
        let mut stack = vec![];
        let mut indentation = 0;

        for entry in &self.log {
            match entry {
                TraceEntry::ValueEvaluated { id, value } => {}
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
                dump.push_str(&format!("{}", arg));
            }
            if let Some(value) = call.return_value {
                dump.push_str(" = ");
                dump.push_str(&format!("{}", value));
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

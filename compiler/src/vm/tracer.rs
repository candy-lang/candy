use super::{heap::Pointer, Heap};
use crate::{
    compiler::{ast_to_hir::AstToHir, cst::CstDb, hir::Id},
    database::Database,
    language_server::utils::LspPositionConversion,
    module::Module,
};
use itertools::Itertools;
use std::time::Instant;
use tracing::error;

#[derive(Clone)]
pub struct Tracer {
    pub events: Vec<Event>,
}

#[derive(Clone)]
pub struct Event {
    pub when: Instant,
    pub data: EventData,
}

#[derive(Clone)]
pub enum EventData {
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
    NeedsEnded {
        nothing: Pointer,
    },
    ParallelStarted,
    ParallelEnded {
        return_value: Pointer,
    },
    ModuleStarted {
        module: Module,
    },
    ModuleEnded {
        export_map: Pointer,
    },
}

impl Tracer {
    pub fn push(&mut self, data: EventData) {
        self.events.push(Event {
            when: Instant::now(),
            data,
        });
    }
}

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
    Parallel,
    Module {
        module: Module,
    },
}

impl Tracer {
    pub fn new() -> Self {
        Self { events: vec![] }
    }
    pub fn stack_trace(&self) -> Vec<StackEntry> {
        let mut stack = vec![];
        for Event { data, .. } in &self.events {
            match data.clone() {
                EventData::ValueEvaluated { .. } => {}
                EventData::CallStarted { id, closure, args } => {
                    stack.push(StackEntry::Call { id, closure, args })
                }
                EventData::CallEnded { .. } => {
                    stack.pop().unwrap();
                }
                EventData::NeedsStarted {
                    id,
                    condition,
                    reason,
                } => stack.push(StackEntry::Needs {
                    id,
                    condition,
                    reason,
                }),
                EventData::NeedsEnded { .. } => {
                    stack.pop().unwrap();
                }
                EventData::ParallelStarted => {
                    stack.push(StackEntry::Parallel);
                }
                EventData::ParallelEnded { .. } => {
                    stack.pop().unwrap();
                }
                EventData::ModuleStarted { module } => stack.push(StackEntry::Module { module }),
                EventData::ModuleEnded { .. } => {
                    stack.pop().unwrap();
                }
            }
        }
        stack
    }
    pub fn format_stack_trace(&self, db: &Database, heap: &Heap) -> String {
        self.stack_trace()
            .iter()
            .rev()
            .map(|entry| {
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
                    StackEntry::Parallel { .. } => {
                        ("parallel section (todo: format children)".to_string(), None)
                    }
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
                format!("{caller_location_string:90} {call_string}")
            })
            .join("\n")
    }
    pub fn dump_stack_trace(&self, db: &Database, heap: &Heap) {
        for line in self.format_stack_trace(db, heap).lines() {
            error!("{}", line);
        }
    }
}

// Full traces are a computed tree view of the whole execution.

pub struct Trace {
    start: Instant,
    end: Instant,
    data: TraceData,
}
pub enum TraceData {
    Call {
        id: Id,
        closure: Pointer,
        args: Vec<Pointer>,
        inner: Vec<Trace>,
        result: TraceResult,
    },
    Needs {
        id: Id,
        condition: Pointer,
        reason: Pointer,
        result: TraceResult,
    },
    Parallel {
        result: TraceResult,
    },
    Module {
        module: Module,
        inner: Vec<Trace>,
        result: TraceResult,
    },
}
pub enum TraceResult {
    Returned(Pointer),
    Panicked(Pointer),
    Canceled,
}

impl Tracer {
    pub fn full_trace(&self) -> Trace {
        let mut stack = vec![Span {
            start: self
                .events
                .first()
                .map(|event| event.when)
                .unwrap_or_else(Instant::now),
            data: None,
            inner: vec![],
        }];
        for event in &self.events {
            match &event.data {
                EventData::ValueEvaluated { .. } => {}
                EventData::CallStarted { id, closure, args } => {
                    stack.push(Span {
                        start: event.when,
                        data: Some(StackEntry::Call {
                            id: id.clone(),
                            closure: *closure,
                            args: args.clone(),
                        }),
                        inner: vec![],
                    });
                }
                EventData::CallEnded { return_value } => {
                    let span = stack.pop().unwrap();
                    let (id, closure, args) = match span.data.unwrap() {
                        StackEntry::Call { id, closure, args } => (id, closure, args),
                        _ => unreachable!(),
                    };
                    stack.last_mut().unwrap().inner.push(Trace {
                        start: span.start,
                        end: event.when,
                        data: TraceData::Call {
                            id,
                            closure,
                            args,
                            inner: span.inner,
                            result: TraceResult::Returned(*return_value),
                        },
                    });
                }
                EventData::NeedsStarted {
                    id,
                    condition,
                    reason,
                } => {
                    stack.push(Span {
                        start: event.when,
                        data: Some(StackEntry::Needs {
                            id: id.clone(),
                            condition: *condition,
                            reason: *reason,
                        }),
                        inner: vec![],
                    });
                }
                EventData::NeedsEnded { nothing } => {
                    let span = stack.pop().unwrap();
                    let (id, condition, reason) = match span.data.unwrap() {
                        StackEntry::Needs {
                            id,
                            condition,
                            reason,
                        } => (id, condition, reason),
                        _ => unreachable!(),
                    };
                    stack.last_mut().unwrap().inner.push(Trace {
                        start: span.start,
                        end: event.when,
                        data: TraceData::Needs {
                            id,
                            condition,
                            reason,
                            result: TraceResult::Returned(*nothing),
                        },
                    });
                }
                EventData::ParallelStarted => {}
                EventData::ParallelEnded { return_value } => {
                    let span = stack.pop().unwrap();
                    stack.last_mut().unwrap().inner.push(Trace {
                        start: span.start,
                        end: event.when,
                        data: TraceData::Parallel {
                            result: TraceResult::Returned(*return_value),
                        },
                    });
                }
                EventData::ModuleStarted { module } => {
                    stack.push(Span {
                        start: event.when,
                        data: Some(StackEntry::Module {
                            module: module.clone(),
                        }),
                        inner: vec![],
                    });
                }
                EventData::ModuleEnded { export_map } => {
                    let span = stack.pop().unwrap();
                    let module = match span.data.unwrap() {
                        StackEntry::Module { module } => module,
                        _ => unreachable!(),
                    };
                    stack.last_mut().unwrap().inner.push(Trace {
                        start: span.start,
                        end: event.when,
                        data: TraceData::Module {
                            module,
                            inner: span.inner,
                            result: TraceResult::Returned(*export_map),
                        },
                    });
                }
            }
        }
        stack.pop().unwrap().inner.pop().unwrap() // TODO: handle multiple traces
    }
    pub fn format_full_trace(&self, heap: &Heap) -> String {
        self.full_trace().format(heap)
    }
}

struct Span {
    start: Instant,
    data: Option<StackEntry>,
    inner: Vec<Trace>,
}

impl TraceResult {
    fn format(&self, heap: &Heap) -> String {
        match self {
            TraceResult::Returned(return_value) => return_value.format(heap),
            TraceResult::Panicked(panic_value) => {
                format!("panicked with {}", panic_value.format(heap))
            }
            TraceResult::Canceled => "canceled".to_string(),
        }
    }
}

impl Trace {
    pub fn format(&self, heap: &Heap) -> String {
        let mut lines = vec![];
        match &self.data {
            TraceData::Call {
                id,
                args,
                inner,
                result,
                ..
            } => {
                lines.push(format!(
                    "call {id} {} = {}",
                    args.iter().map(|arg| arg.format(heap)).join(" "),
                    result.format(heap),
                ));
                for trace in inner {
                    lines.extend(trace.format(heap).lines().map(|line| format!("  {line}")));
                }
            }
            TraceData::Needs {
                condition,
                reason,
                result,
                ..
            } => {
                lines.push(format!(
                    "needs {} {} = {}",
                    condition.format(heap),
                    reason.format(heap),
                    result.format(heap),
                ));
            }
            TraceData::Parallel { result } => {
                lines.push(format!(
                    "parallel section that completed with {}",
                    result.format(heap),
                ));
            }
            TraceData::Module {
                module,
                inner,
                result,
            } => {
                lines.push(format!("{module} = {}", result.format(heap)));
                for trace in inner {
                    lines.extend(trace.format(heap).lines().map(|line| format!("  {line}")));
                }
            }
        }
        lines.join("\n")
    }
}

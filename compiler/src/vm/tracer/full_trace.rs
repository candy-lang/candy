use super::{
    super::heap::{ChannelId, Pointer},
    FiberId, Heap,
};
use crate::{
    compiler::{ast_to_hir::AstToHir, cst::CstDb, hir::Id},
    database::Database,
    language_server::utils::LspPositionConversion,
    module::Module,
};
use itertools::Itertools;
use std::{collections::HashMap, time::Instant};
use tracing::error;

// Full traces are a computed tree view of the whole execution.

pub struct Trace {
    #[allow(dead_code)]
    start: Instant,

    #[allow(dead_code)]
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
    Module {
        module: Module,
        inner: Vec<Trace>,
        result: TraceResult,
    },
}
pub enum TraceResult {
    Returned(Pointer),

    #[allow(dead_code)]
    Panicked(Pointer),

    #[allow(dead_code)]
    Canceled,
}

impl FullTracer {
    pub fn full_trace(&self) -> Trace {
        let mut stacks: HashMap<FiberId, Vec<Span>> = HashMap::new();
        for TimedEvent { when, event } in &self.events {
            match &event {
                Event::FiberCreated { fiber } => {
                    stacks.insert(
                        *fiber,
                        vec![Span {
                            start: when.clone(),
                            data: None,
                            inner: vec![],
                        }],
                    );
                }
                Event::InFiber { fiber, event } => {
                    let stack = stacks.get_mut(fiber).unwrap();
                    match event {
                        InFiberEvent::ModuleStarted { module } => todo!(),
                        InFiberEvent::ModuleEnded { export_map } => todo!(),
                        InFiberEvent::CallStarted { id, closure, args } => stack.push(Span {
                            start: when,
                            data: Some(StackEntry::Call {
                                id: id.clone(),
                                closure: *closure,
                                args: args.clone(),
                            }),
                            inner: vec![],
                        }),
                        InFiberEvent::CallEnded { return_value } => {
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
                        InFiberEvent::ModuleStarted { module } => {
                            stack.push(Span {
                                start: event.when,
                                data: Some(StackEntry::Module {
                                    module: module.clone(),
                                }),
                                inner: vec![],
                            });
                        }
                        InFiberEvent::ModuleEnded { export_map } => {
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
                        _ => {}
                    }
                }
            }
        }
        stacks
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

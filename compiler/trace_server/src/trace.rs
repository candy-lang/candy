use std::fmt::Debug;

use candy_frontend::id::CountableId;
use candy_vm::heap::{Heap, Pointer};
use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::{storage::TraceStorage, time::Time};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct TraceId(usize);

#[derive(Clone)]
pub enum Trace {
    CallSpan {
        call_site: Pointer,
        callee: Pointer,
        arguments: Vec<Pointer>,
        children: Option<Vec<TraceId>>,
        start: Time,
        end: Option<CallEnd>,
    },
    ValueEvaluated {
        expression: Pointer,
        value: Pointer,
    },
}
#[derive(Clone, Copy)]
pub struct CallEnd {
    pub when: Time,
    pub result: CallResult,
}
#[derive(Clone, Copy)]
pub enum CallResult {
    Canceled,
    Panicked,
    Returned(Pointer),
}

impl CountableId for TraceId {
    fn from_usize(id: usize) -> Self {
        TraceId(id)
    }

    fn to_usize(&self) -> usize {
        self.0
    }
}

impl Trace {
    pub fn format(&self, storage: &TraceStorage) -> String {
        match self {
            Trace::CallSpan {
                call_site,
                callee,
                arguments,
                children,
                start,
                end,
            } => {
                format!(
                    "{} {} -> {}{}",
                    callee.format(&storage.heap),
                    arguments
                        .iter()
                        .map(|argument| argument.format(&storage.heap))
                        .join(" "),
                    match end {
                        None => "?".to_string(),
                        Some(CallEnd { when, result }) => match result {
                            CallResult::Canceled => "canceled".to_string(),
                            CallResult::Panicked => "panicked".to_string(),
                            CallResult::Returned(value) => value.format(&storage.heap),
                        },
                    },
                    match &children {
                        Some(children) => {
                            children
                                .iter()
                                .map(|child| format!("\n{}", storage.get(*child).format(storage)))
                                .join("")
                                .lines()
                                .map(|line| format!("  {line}"))
                                .join("\n")
                        }
                        None => "\n  (can be lazily re-computed)".to_string(),
                    }
                )
            }
            Trace::ValueEvaluated { expression, value } => todo!(),
        }
    }
}

impl Trace {
    pub fn change_pointers(&mut self, pointer_map: &FxHashMap<Pointer, Pointer>) {
        match self {
            Trace::CallSpan {
                call_site,
                callee,
                arguments,
                children: _,
                start,
                end,
            } => {
                *call_site = pointer_map.get(call_site).copied().unwrap_or(*call_site);
                *callee = pointer_map.get(callee).copied().unwrap_or(*callee);
                for argument in arguments {
                    *argument = pointer_map.get(argument).copied().unwrap_or(*argument);
                }
                if let Some(CallEnd {
                    result: CallResult::Returned(value),
                    ..
                }) = end
                {
                    *value = pointer_map.get(value).copied().unwrap_or(*value);
                }
            }
            Trace::ValueEvaluated { expression, value } => {
                *expression = pointer_map.get(expression).copied().unwrap_or(*expression);
                *value = pointer_map.get(value).copied().unwrap_or(*value);
            }
        }
    }
}

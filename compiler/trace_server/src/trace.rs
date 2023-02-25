use std::fmt::Debug;

use candy_vm::heap::{Heap, Pointer};
use itertools::Itertools;

#[derive(Clone)]
pub struct Trace {
    pub heap: Heap,
    pub root: CallSpan,
}

#[derive(Clone)]
pub struct CallSpan {
    pub call_site: Pointer,
    pub callee: Pointer,
    pub arguments: Vec<Pointer>,
    pub children: Option<Vec<CallSpan>>,
    pub end: End,
}
#[derive(Clone, Copy)]
pub enum End {
    NotYet,
    Canceled,
    Panicked,
    Returns(Pointer),
}

impl Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.root.format(&self.heap))
    }
}
impl CallSpan {
    fn format(&self, heap: &Heap) -> String {
        format!(
            "{} {} -> {}{}",
            self.callee.format(heap),
            self.arguments
                .iter()
                .map(|argument| argument.format(heap))
                .join(" "),
            match self.end {
                End::NotYet => "?".to_string(),
                End::Canceled => "canceled".to_string(),
                End::Panicked => "panicked".to_string(),
                End::Returns(value) => value.format(heap),
            },
            match &self.children {
                Some(children) => {
                    children
                        .iter()
                        .map(|child| format!("\n{}", child.format(heap)))
                        .join("")
                        .lines()
                        .map(|line| format!("  {line}"))
                        .join("\n")
                }
                None => "\n  (can be lazily re-computed)".to_string(),
            }
        )
    }
}

impl CallSpan {
    fn map_pointers(&mut self, pointer_map: &FxHashMap<Pointer, Pointer>) {
        let CallSpan {
            call_site,
            callee,
            arguments,
            children,
            end,
        } = &mut self;

        *call_site = pointer_map.get(call_site);
        *callee = pointer_map.get(callee);
        for argument in arguments {
            *argument = pointer_map.get(argument);
        }
        if let Some(children) = children {
            for child in children {
                child.map_pointers(pointer_map);
            }
        }
    }
}

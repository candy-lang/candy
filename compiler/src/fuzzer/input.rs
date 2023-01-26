use std::fmt;

use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::vm::{Heap, Pointer};

#[derive(Clone)]
pub struct Input {
    pub heap: Heap,
    pub arguments: Vec<Pointer>,
}
impl Input {
    pub fn clone_to_other_heap(&self, other: &mut Heap) -> Vec<Pointer> {
        self.heap
            .clone_multiple_to_other_heap_with_existing_mapping(
                other,
                &self.arguments,
                &mut FxHashMap::default(),
            )
    }
}
impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.arguments
                .iter()
                .map(|arg| arg.format(&self.heap))
                .join(" "),
        )
    }
}

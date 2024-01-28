use candy_frontend::format::{MaxLength, Precedence};
use candy_vm::heap::{Heap, HeapObject, InlineObject, ToDebugText};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Display, Formatter},
    hash::Hash,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Input {
    arguments: Vec<InlineObject>,
}
impl Input {
    #[must_use]
    pub fn new(arguments: Vec<InlineObject>) -> Self {
        Self { arguments }
    }

    #[must_use]
    pub fn arguments(&self) -> &[InlineObject] {
        &self.arguments
    }

    pub fn dup(&self, heap: &mut Heap) {
        for argument in &self.arguments {
            argument.dup(heap);
        }
    }
    pub fn drop(&self, heap: &mut Heap) {
        for argument in &self.arguments {
            argument.drop(heap);
        }
    }
    #[must_use]
    pub fn clone_to_heap_with_mapping(
        &self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        Self::new(
            self.arguments
                .iter()
                .map(|argument| argument.clone_to_heap_with_mapping(heap, address_map))
                .collect(),
        )
    }
}

impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.arguments
                .iter()
                .map(|argument| argument.to_debug_text(Precedence::High, MaxLength::Limited(40)))
                .join(" "),
        )
    }
}

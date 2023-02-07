use std::{cell::RefCell, fmt, hash, rc::Rc};

use itertools::Itertools;
use rustc_hash::FxHashMap;

use candy_vm::heap::{Heap, Pointer};

#[derive(Clone)]
pub struct Input {
    pub heap: Rc<RefCell<Heap>>,
    pub arguments: Vec<Pointer>,
}
impl Input {
    pub fn clone_to_other_heap(&self, other: &mut Heap) -> Vec<Pointer> {
        self.heap
            .borrow()
            .clone_multiple_to_other_heap_with_existing_mapping(
                other,
                &self.arguments,
                &mut FxHashMap::default(),
            )
    }
}
impl hash::Hash for Input {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        let heap = self.heap.borrow();
        for argument in &self.arguments {
            let value = (*argument).hash(&heap);
            state.write_u64(value);
        }
    }
}
impl PartialEq for Input {
    /// This function assumes that the other input uses the same underlying
    /// heap. This assumption should hold because all inputs generated during a
    /// fuzzing run are saved in the same heap.
    fn eq(&self, other: &Self) -> bool {
        assert!(Rc::ptr_eq(&self.heap, &other.heap));
        if self.arguments.len() != other.arguments.len() {
            return false;
        }
        let heap = self.heap.borrow();
        self.arguments
            .iter()
            .zip(&other.arguments)
            .all(|(a, b)| a.equals(&heap, *b))
    }
}
impl Eq for Input {}
impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let heap = self.heap.borrow();
        write!(
            f,
            "{}",
            self.arguments.iter().map(|arg| arg.format(&heap)).join(" "),
        )
    }
}

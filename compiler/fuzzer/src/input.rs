use candy_frontend::format::{MaxLength, Precedence};
use candy_vm::heap::{DisplayWithSymbolTable, Heap, InlineObject, SymbolTable, ToDebugText};
use itertools::Itertools;
use std::{
    cell::RefCell,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    rc::Rc,
};

#[derive(Clone)]
pub struct Input {
    pub heap: Rc<RefCell<Heap>>,
    pub arguments: Vec<InlineObject>,
}

impl DisplayWithSymbolTable for Input {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        write!(
            f,
            "{}",
            self.arguments
                .iter()
                .map(|argument| argument.to_debug_text(
                    Precedence::High,
                    MaxLength::Limited(40),
                    symbol_table
                ))
                .join(" "),
        )
    }
}

impl Eq for Input {}
impl PartialEq for Input {
    /// This function assumes that the other input uses the same underlying
    /// heap. This assumption should hold because all inputs generated during a
    /// fuzzing run are saved in the same heap.
    fn eq(&self, other: &Self) -> bool {
        assert!(Rc::ptr_eq(&self.heap, &other.heap));
        self.arguments == other.arguments
    }
}
impl Hash for Input {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.arguments.hash(state)
    }
}

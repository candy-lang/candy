use candy_vm::heap::{Heap, InlineObject};
use itertools::Itertools;
use std::{
    cell::RefCell,
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
    rc::Rc,
};

#[derive(Clone)]
pub struct Input<'h> {
    pub heap: Rc<RefCell<Heap<'h>>>,
    pub arguments: Vec<InlineObject<'h>>,
}

impl Display for Input<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.arguments.iter().map(|it| format!("{it:?}")).join(" "),
        )
    }
}

impl Eq for Input<'_> {}
impl PartialEq for Input<'_> {
    /// This function assumes that the other input uses the same underlying
    /// heap. This assumption should hold because all inputs generated during a
    /// fuzzing run are saved in the same heap.
    fn eq(&self, other: &Self) -> bool {
        assert!(Rc::ptr_eq(&self.heap, &other.heap));
        self.arguments == other.arguments
    }
}
impl Hash for Input<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.arguments.hash(state)
    }
}

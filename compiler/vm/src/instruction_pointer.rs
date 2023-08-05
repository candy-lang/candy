use derive_more::{Deref, From};
use std::{
    fmt::{self, Debug},
    iter::Step,
};

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionPointer(usize);
impl InstructionPointer {
    pub fn null_pointer() -> Self {
        Self(0)
    }
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}
impl Step for InstructionPointer {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(**end - **start)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        (*start).checked_add(count).map(Self)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        (*start).checked_sub(count).map(Self)
    }
}
impl Debug for InstructionPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ip-{}", self.0)
    }
}
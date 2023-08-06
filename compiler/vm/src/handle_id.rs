use candy_frontend::id::CountableId;
use std::fmt::{self, Debug};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandleId(usize);

impl CountableId for HandleId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}
impl Debug for HandleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "handle_{:x}", self.0)
    }
}

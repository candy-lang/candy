use crate::id::CountableId;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub usize);

impl CountableId for Id {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }

    fn to_usize(&self) -> usize {
        self.0
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "CstId({})", self.0)
    }
}

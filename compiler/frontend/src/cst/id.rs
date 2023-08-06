use crate::impl_countable_id;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub usize);

impl_countable_id!(Id);

impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "CstId({})", self.0)
    }
}

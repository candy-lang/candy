use crate::{
    impl_countable_id,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use std::fmt::{self, Debug, Display, Formatter};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(usize);

impl_countable_id!(Id);

impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
impl Debug for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
impl ToRichIr for Id {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Variable, EnumSet::empty());
        builder.push_reference(*self, range);
    }
}

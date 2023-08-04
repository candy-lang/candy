use crate::{
    id::CountableId,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use std::fmt::{self, Debug, Display, Formatter};

#[derive(Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Id(usize);
impl Id {
    #[must_use]
    pub fn to_short_debug_string(&self) -> String {
        format!("${}", self.0)
    }
}

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
        write!(f, "{}", self.to_short_debug_string())
    }
}
impl Debug for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.to_short_debug_string())
    }
}
impl ToRichIr for Id {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            self.to_short_debug_string(),
            TokenType::Variable,
            EnumSet::empty(),
        );
        builder.push_reference(*self, range);
    }
}

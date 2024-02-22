use super::{Body, Constants, Expression};
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
impl Id {
    pub fn build_rich_ir_with_constants(
        self,
        builder: &mut RichIrBuilder,
        constants: impl Into<Option<&Constants>>,
        body: impl Into<Option<&Body>>,
    ) {
        self.build_rich_ir(builder);
        if let Some(body) = body.into()
            && let Some(Expression::Constant(constant_id)) = body.expression(self)
        {
            builder.push("<", None, EnumSet::empty());
            constant_id.build_rich_ir_with_constants(builder, constants);
            builder.push(">", None, EnumSet::empty());
        }
    }
}
impl ToRichIr for Id {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Variable, EnumSet::empty());
        builder.push_reference(*self, range);
    }
}

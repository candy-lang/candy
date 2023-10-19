pub use self::{body::*, constant::*, expression::*, id::*};
use crate::rich_ir::{RichIrBuilder, ToRichIr, TokenType};
use enumset::EnumSet;

mod body;
mod constant;
mod expression;
mod id;

// TODO: `impl Hash for Lir`
// TODO: `impl ToRichIr for Lir`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lir {
    constants: Constants,
    bodies: Bodies,
}
impl Lir {
    #[must_use]
    pub const fn new(constants: Constants, bodies: Bodies) -> Self {
        Self { constants, bodies }
    }

    #[must_use]
    pub const fn constants(&self) -> &Constants {
        &self.constants
    }
    #[must_use]
    pub const fn bodies(&self) -> &Bodies {
        &self.bodies
    }
}

impl ToRichIr for Lir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push("# Constants", TokenType::Comment, EnumSet::empty());
        builder.push_newline();
        self.constants.build_rich_ir(builder);
        builder.push_newline();
        builder.push_newline();

        builder.push("# Bodies", TokenType::Comment, EnumSet::empty());
        builder.push_newline();
        self.bodies.build_rich_ir(builder);
    }
}

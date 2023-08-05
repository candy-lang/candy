pub use self::{body::*, constant::*, expression::*, id::*};

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
    pub fn new(constants: Constants, bodies: Bodies) -> Self {
        Self { constants, bodies }
    }
}

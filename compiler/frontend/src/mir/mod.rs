pub use self::{body::*, expression::*, id::*};
use crate::{
    id::IdGenerator,
    rich_ir::{RichIrBuilder, ToRichIr},
};
use std::fmt::{self, Debug, Formatter};

mod body;
mod expression;
mod id;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Mir {
    pub id_generator: IdGenerator<Id>,
    pub body: Body,
}
impl Mir {
    pub fn build<F>(function: F) -> Self
    where
        F: FnOnce(&mut BodyBuilder),
    {
        let mut builder = BodyBuilder::new(IdGenerator::start_at(0));
        function(&mut builder);
        let (id_generator, body) = builder.finish();
        Mir { id_generator, body }
    }
}
impl Debug for Mir {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body.to_rich_ir())
    }
}
impl ToRichIr for Mir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.body.build_rich_ir(builder);
    }
}

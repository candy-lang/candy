pub use self::{body::*, error::*, expression::*, id::*};
use crate::{
    id::IdGenerator,
    impl_debug_via_richir, impl_display_via_richir,
    rich_ir::{RichIrBuilder, ToRichIr},
};

mod body;
mod error;
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
        let mut builder = BodyBuilder::new(IdGenerator::start_at(1));
        function(&mut builder);
        let (id_generator, body) = builder.finish();
        Self { id_generator, body }
    }
}
impl_debug_via_richir!(Mir);
impl_display_via_richir!(Mir);
impl ToRichIr for Mir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.body.build_rich_ir(builder);
    }
}

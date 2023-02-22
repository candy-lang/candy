use std::fmt::{self, Debug, Formatter};

use derive_more::From;
use num_bigint::BigInt;

use crate::{
    builtin_functions::BuiltinFunction,
    hir,
    id::IdGenerator,
    module::Module,
    rich_ir::{RichIrBuilder, ToRichIr},
};

pub use self::{body::*, expression::*, id::*};

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
impl ToRichIr<MirReferenceKey> for Mir {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder<MirReferenceKey>) {
        self.body.build_rich_ir(builder);
    }
}

#[derive(Debug, PartialEq, Eq, Hash, From)]
pub enum MirReferenceKey {
    Module(Module),
    Id(Id),
    Int(BigInt),
    Text(String),
    #[from(ignore)]
    Symbol(String),
    BuiltinFunction(BuiltinFunction),
    HirId(hir::Id),
}

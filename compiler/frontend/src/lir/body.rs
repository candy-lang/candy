use super::Expression;
use crate::{
    hir,
    id::CountableId,
    impl_countable_id,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use rustc_hash::FxHashSet;
use std::fmt::{self, Debug, Display, Formatter};

// ID

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BodyId(usize);

impl_countable_id!(BodyId);

impl Debug for BodyId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "body_{}", self.0)
    }
}
impl Display for BodyId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "body_{}", self.0)
    }
}
impl ToRichIr for BodyId {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(self.to_string(), TokenType::Function, EnumSet::empty());
        builder.push_reference(*self, range);
    }
}

// Bodies

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bodies(Vec<Body>);

impl Bodies {
    pub fn push(&mut self, constant: Body) -> BodyId {
        let id = BodyId::from_usize(self.0.len());
        self.0.push(constant);
        id
    }
    pub fn get(&self, id: BodyId) -> &Body {
        &self.0[id.to_usize()]
    }
}

// Body

/// IDs are assigned sequentially in the following order, starting at 0:
///
/// - captured variables
/// - parameters
/// - responsible parameter
/// - locals
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Body {
    original_hirs: FxHashSet<hir::Id>,
    captured_count: usize,
    parameter_count: usize,
    expressions: Vec<Expression>,
}
impl Body {
    pub fn new(
        original_hirs: FxHashSet<hir::Id>,
        captured_count: usize,
        parameter_count: usize,
        expressions: Vec<Expression>,
    ) -> Self {
        Self {
            original_hirs,
            captured_count,
            parameter_count,
            expressions,
        }
    }

    pub fn original_hirs(&self) -> &FxHashSet<hir::Id> {
        &self.original_hirs
    }
    pub fn captured_count(&self) -> usize {
        self.captured_count
    }
    pub fn parameter_count(&self) -> usize {
        self.parameter_count
    }
    pub fn expressions(&self) -> &[Expression] {
        &self.expressions
    }
}

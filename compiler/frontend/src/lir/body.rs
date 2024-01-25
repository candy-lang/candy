use super::{Constants, Expression, Id};
use crate::{
    hir,
    id::CountableId,
    impl_countable_id,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use itertools::Itertools;
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
    #[must_use]
    pub fn get(&self, id: BodyId) -> &Body {
        &self.0[id.to_usize()]
    }
    pub fn push(&mut self, body: Body) -> BodyId {
        let id = BodyId::from_usize(self.0.len());
        self.0.push(body);
        id
    }

    pub fn ids_and_bodies(&self) -> impl Iterator<Item = (BodyId, &Body)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, it)| (BodyId(index), it))
    }

    pub fn build_rich_ir_with_constants(
        &self,
        builder: &mut RichIrBuilder,
        constants: impl Into<Option<&Constants>>,
    ) {
        let constants = constants.into();
        builder.push_custom_multiline(self.ids_and_bodies(), |builder, (id, body)| {
            let range = builder.push(id.to_string(), TokenType::Function, EnumSet::empty());

            builder.push_definition(*id, range);
            for parameter_id in body.parameter_ids() {
                builder.push(" ", None, EnumSet::empty());
                let range = builder.push(
                    parameter_id.to_string(),
                    TokenType::Parameter,
                    EnumSet::empty(),
                );
                builder.push_definition(parameter_id, range);
            }

            builder.push(" =", None, EnumSet::empty());

            builder.indent();
            builder.push_newline();
            body.build_rich_ir_with_constants(builder, constants);
            builder.dedent();
        });
    }
}
impl ToRichIr for Bodies {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.build_rich_ir_with_constants(builder, None);
    }
}

// Body

/// IDs are assigned sequentially in the following order, starting at 0:
///
/// - captured variables
/// - parameters
/// - locals
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Body {
    original_hirs: FxHashSet<hir::Id>,
    captured_count: usize,
    parameter_count: usize,
    expressions: Vec<Expression>,
}
impl Body {
    #[must_use]
    pub fn new(
        original_hirs: FxHashSet<hir::Id>,
        captured_count: usize,
        parameter_count: usize,
    ) -> Self {
        Self {
            original_hirs,
            captured_count,
            parameter_count,
            expressions: vec![],
        }
    }

    #[must_use]
    pub const fn original_hirs(&self) -> &FxHashSet<hir::Id> {
        &self.original_hirs
    }

    #[must_use]
    pub const fn captured_count(&self) -> usize {
        self.captured_count
    }
    pub fn captured_ids(&self) -> impl Iterator<Item = Id> {
        (0..self.captured_count).map(Id::from_usize)
    }

    #[must_use]
    pub const fn parameter_count(&self) -> usize {
        self.parameter_count
    }
    pub fn parameter_ids(&self) -> impl Iterator<Item = Id> {
        (self.captured_count..self.captured_count + self.parameter_count).map(Id::from_usize)
    }

    #[must_use]
    pub fn first_expression_id(&self) -> Id {
        Id::from_usize(self.captured_count + self.parameter_count)
    }

    #[must_use]
    pub fn expressions(&self) -> &[Expression] {
        &self.expressions
    }
    #[must_use]
    pub fn expression(&self, id: Id) -> Option<&Expression> {
        let expression_id_offset = self.expression_id_offset();
        if id.to_usize() < expression_id_offset {
            return None;
        }

        self.expressions.get(id.to_usize() - expression_id_offset)
    }
    #[must_use]
    pub fn ids_and_expressions(&self) -> impl DoubleEndedIterator<Item = (Id, &Expression)> {
        let offset = self.expression_id_offset();
        self.expressions
            .iter()
            .enumerate()
            .map(move |(index, it)| (Id::from_usize(offset + index), it))
    }
    #[must_use]
    pub fn last_expression_id(&self) -> Option<Id> {
        if self.expressions.is_empty() {
            None
        } else {
            Some(Id::from_usize(
                self.expression_id_offset() + self.expressions.len() - 1,
            ))
        }
    }
    #[must_use]
    const fn expression_id_offset(&self) -> usize {
        self.captured_count + self.parameter_count
    }

    pub fn push(&mut self, expression: Expression) -> Id {
        self.expressions.push(expression);
        self.last_expression_id().unwrap()
    }

    pub fn build_rich_ir_with_constants(
        &self,
        builder: &mut RichIrBuilder,
        constants: impl Into<Option<&Constants>>,
    ) {
        let constants = constants.into();
        builder.push("# Original HIR IDs: ", TokenType::Comment, EnumSet::empty());
        builder.push_children_custom(
            self.original_hirs.iter().sorted(),
            |builder, id| {
                let range = builder.push(id.to_string(), TokenType::Symbol, EnumSet::empty());
                builder.push_reference((*id).clone(), range);
            },
            ", ",
        );
        builder.push_newline();

        builder.push("# Captured IDs: ", TokenType::Comment, EnumSet::empty());
        if self.captured_ids().next().is_none() {
            builder.push("none", None, EnumSet::empty());
        } else {
            builder.push_children_custom(
                self.captured_ids().collect_vec(),
                |builder, id| {
                    let range = builder.push(id.to_string(), TokenType::Variable, EnumSet::empty());
                    builder.push_definition(*id, range);
                },
                ", ",
            );
        }
        builder.push_newline();

        builder.push_custom_multiline(self.ids_and_expressions(), |builder, (id, expression)| {
            let range = builder.push(id.to_string(), TokenType::Variable, EnumSet::empty());
            builder.push_definition(*id, range);
            builder.push(" = ", None, EnumSet::empty());
            expression.build_rich_ir_with_constants(builder, constants, self);
        });
    }
}
impl ToRichIr for Body {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        self.build_rich_ir_with_constants(builder, None);
    }
}

use super::{data_flow::DataFlow, pure::PurenessInsights, OptimizeMir};
use crate::{
    error::CompilerError,
    id::IdGenerator,
    mir::{Body, Expression, Id, VisibleExpressions},
    TracingConfig,
};
use rustc_hash::FxHashSet;
use std::ops::{Deref, DerefMut};

pub struct Context<'a> {
    pub db: &'a dyn OptimizeMir,
    pub tracing: &'a TracingConfig,
    pub errors: FxHashSet<CompilerError>,
    pub visible: VisibleExpressions,
    pub id_generator: &'a mut IdGenerator<Id>,
    pub pureness: PurenessInsights,
    pub data_flow: DataFlow,
}
impl<'a> Context<'a> {
    pub fn new(
        db: &'a dyn OptimizeMir,
        tracing: &'a TracingConfig,
        errors: FxHashSet<CompilerError>,
        id_generator: &'a mut IdGenerator<Id>,
        body: &Body,
    ) -> Self {
        Self {
            db,
            tracing,
            errors,
            visible: VisibleExpressions::none_visible(),
            id_generator,
            pureness: PurenessInsights::default(),
            data_flow: DataFlow::new(body),
        }
    }
}

pub struct CurrentExpression<'a> {
    body: &'a mut Body,
    index: usize,
}
impl<'a> CurrentExpression<'a> {
    pub fn new(body: &'a mut Body, index: usize) -> Self {
        Self { body, index }
    }

    pub const fn index(&self) -> usize {
        self.index
    }
    pub fn id(&self) -> Id {
        self.body.expressions[self.index].0
    }

    pub fn prepend_optimized(
        &mut self,
        visible: &mut VisibleExpressions,
        optimized_expressions: impl IntoIterator<Item = (Id, Expression)>,
    ) {
        self.body.expressions.splice(
            self.index..self.index,
            optimized_expressions.into_iter().map(|(id, expression)| {
                visible.insert(id, expression);
                self.index += 1;
                (id, Expression::Parameter)
            }),
        );
    }
    pub fn replace_with_multiple<I: DoubleEndedIterator<Item = (Id, Expression)>>(
        &mut self,
        expressions: impl IntoIterator<Item = (Id, Expression), IntoIter = I>,
    ) {
        let mut expressions = expressions.into_iter();
        let (_, last_expression) = expressions.next_back().unwrap();
        self.body.expressions.splice(
            self.index..=self.index,
            expressions.chain([(self.id(), last_expression)]),
        );
    }
}

impl Deref for CurrentExpression<'_> {
    type Target = Expression;

    fn deref(&self) -> &Self::Target {
        &self.body.expressions[self.index].1
    }
}
impl DerefMut for CurrentExpression<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body.expressions[self.index].1
    }
}

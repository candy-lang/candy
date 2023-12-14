use super::{pure::PurenessInsights, OptimizeMir};
use crate::{
    error::CompilerError,
    id::IdGenerator,
    mir::{Body, Expression, Id, VisibleExpressions},
    TracingConfig,
};
use rustc_hash::FxHashSet;
use std::ops::Deref;

pub struct Context<'a> {
    pub db: &'a dyn OptimizeMir,
    pub tracing: &'a TracingConfig,
    pub errors: &'a mut FxHashSet<CompilerError>,
    pub visible: &'a mut VisibleExpressions,
    pub id_generator: &'a mut IdGenerator<Id>,
    pub pureness: &'a mut PurenessInsights,
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

    pub fn get_mut_carefully(&mut self) -> &mut Expression {
        &mut self.body.expressions[self.index].1
    }
    pub fn replace_id_references(&mut self, replacer: &mut impl FnMut(&mut Id)) {
        self.get_mut_carefully().replace_id_references(replacer);
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
    pub fn replace_with(
        &mut self,
        expression: Expression,
        pureness: &mut PurenessInsights,
    ) -> Expression {
        let id = self.body.expressions[self.index].0;
        self.replace_with_multiple([(id, expression)], pureness)
    }
    pub fn replace_with_multiple<I: DoubleEndedIterator<Item = (Id, Expression)>>(
        &mut self,
        expressions: impl IntoIterator<Item = (Id, Expression), IntoIter = I>,
        pureness: &mut PurenessInsights,
    ) -> Expression {
        let mut expressions = expressions.into_iter();
        let (_, last_expression) = expressions.next_back().unwrap();
        let mut removed = self.body.expressions.splice(
            self.index..=self.index,
            expressions.chain([(self.id(), last_expression)]),
        );
        let (_, removed_expression) = removed.next().unwrap();
        assert!(removed.next().is_none());
        for id in removed_expression.defined_ids() {
            pureness.on_remove(id);
        }
        removed_expression
    }
}

impl Deref for CurrentExpression<'_> {
    type Target = Expression;

    fn deref(&self) -> &Self::Target {
        &self.body.expressions[self.index].1
    }
}

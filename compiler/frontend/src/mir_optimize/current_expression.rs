use crate::mir::{Body, Expression, Id, VisibleExpressions};
use std::ops::{Deref, DerefMut};

pub struct ExpressionContext<'a> {
    pub visible: &'a mut VisibleExpressions,
    pub expression: CurrentExpression<'a>,
}
impl<'a> ExpressionContext<'a> {
    pub fn prepend_optimized(
        &mut self,
        optimized_expressions: impl IntoIterator<Item = (Id, Expression)>,
    ) {
        self.expression.body.expressions.splice(
            self.expression.index..self.expression.index,
            optimized_expressions.into_iter().map(|(id, expression)| {
                self.visible.insert(id, expression);
                self.expression.index += 1;
                (id, Expression::Parameter)
            }),
        );
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

    pub fn index(&self) -> usize {
        self.index
    }
    pub fn id(&self) -> Id {
        self.body.expressions[self.index].0
    }

    pub fn replace_with_multiple<I: DoubleEndedIterator<Item = (Id, Expression)>>(
        &mut self,
        expressions: impl IntoIterator<Item = (Id, Expression), IntoIter = I>,
    ) {
        // FIXME: Update call sites.
        let mut expressions = expressions.into_iter();
        let (_, last_expression) = expressions.next_back().unwrap();
        self.body.expressions.splice(
            self.index..(self.index + 1),
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

use super::{expression::Expression, id::Id, MirReferenceKey};
use crate::{
    builtin_functions::BuiltinFunction,
    hir,
    id::{CountableId, IdGenerator},
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use derive_more::{Deref, DerefMut};
use enumset::EnumSet;
use itertools::Itertools;
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Formatter},
    mem, vec,
};

#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Body {
    expressions: Vec<(Id, Expression)>,
}
impl Body {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Id, &Expression)> {
        self.expressions
            .iter()
            .map(|(id, expression)| (*id, expression))
    }
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (Id, &mut Expression)> {
        self.expressions
            .iter_mut()
            .map(|(id, expression)| (*id, expression))
    }
}
impl IntoIterator for Body {
    type Item = (Id, Expression);
    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.expressions.into_iter()
    }
}
impl Body {
    pub fn return_value(&mut self) -> Id {
        let (id, _) = self.expressions.last().unwrap();
        *id
    }

    pub fn push(&mut self, id: Id, expression: Expression) {
        self.expressions.push((id, expression));
    }
    pub fn push_with_new_id(
        &mut self,
        id_generator: &mut IdGenerator<Id>,
        expression: Expression,
    ) -> Id {
        let id = id_generator.generate();
        self.push(id, expression);
        id
    }
    pub fn insert_at_front(&mut self, expressions: Vec<(Id, Expression)>) {
        let old_expressions = mem::take(&mut self.expressions);
        self.expressions.extend(expressions);
        self.expressions.extend(old_expressions);
    }
    pub fn remove_all<F>(&mut self, mut predicate: F)
    where
        F: FnMut(Id, &Expression) -> bool,
    {
        self.expressions
            .retain(|(id, expression)| !predicate(*id, expression));
    }
    pub fn sort_by<F>(&mut self, predicate: F)
    where
        F: FnMut(&(Id, Expression), &(Id, Expression)) -> Ordering,
    {
        self.expressions.sort_by(predicate);
    }

    /// Flattens all `Expression::Multiple`.
    pub fn flatten_multiples(&mut self) {
        let old_expressions = mem::take(&mut self.expressions);

        for (id, mut expression) in old_expressions.into_iter() {
            if let Expression::Multiple(mut inner_body) = expression {
                inner_body.flatten_multiples();
                let returned_by_inner = inner_body.return_value();
                for (id, expression) in inner_body.expressions {
                    self.expressions.push((id, expression));
                }
                self.expressions
                    .push((id, Expression::Reference(returned_by_inner)));
            } else {
                if let Expression::Lambda { body, .. } = &mut expression {
                    body.flatten_multiples();
                }
                self.expressions.push((id, expression));
            }
        }
    }
}
#[test]
fn test_multiple_flattening() {
    use crate::{
        builtin_functions::BuiltinFunction,
        mir::{Expression, Mir},
    };

    // $0 =
    //   $1 = builtinEquals
    //
    // # becomes:
    // $0 = builtinEquals
    // $1 = $0
    let mut mir = Mir::build(|body| {
        body.push_multiple(|body| {
            body.push(Expression::Builtin(BuiltinFunction::Equals));
        });
    });
    mir.flatten_multiples();
    mir.normalize_ids();
    assert_eq!(
        mir,
        Mir::build(|body| {
            let inlined = body.push(Expression::Builtin(BuiltinFunction::Equals));
            body.push(Expression::Reference(inlined));
        }),
    );
}

pub enum VisitorResult {
    Continue,
    Abort,
}

#[derive(Clone)]
pub struct VisibleExpressions {
    expressions: FxHashMap<Id, Expression>,
}
impl VisibleExpressions {
    pub fn none_visible() -> Self {
        Self {
            expressions: FxHashMap::default(),
        }
    }
    pub fn insert(&mut self, id: Id, expression: Expression) {
        self.expressions.insert(id, expression);
    }
    pub fn get(&self, id: Id) -> &Expression {
        self.expressions.get(&id).unwrap()
    }
    pub fn contains(&self, id: Id) -> bool {
        self.expressions.contains_key(&id)
    }
}
impl Debug for VisibleExpressions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.expressions
                .keys()
                .sorted()
                .map(|id| id.to_rich_ir().text)
                .join(", "),
        )
    }
}

impl Body {
    pub fn visit(
        &mut self,
        visitor: &mut dyn FnMut(Id, &mut Expression, bool) -> VisitorResult,
    ) -> VisitorResult {
        let length = self.expressions.len();
        for i in 0..length {
            let (id, expression) = self.expressions.get_mut(i).unwrap();
            match Self::visit_expression(*id, expression, i == length - 1, visitor) {
                VisitorResult::Continue => {}
                VisitorResult::Abort => return VisitorResult::Abort,
            }
        }
        VisitorResult::Continue
    }
    fn visit_expression(
        id: Id,
        expression: &mut Expression,
        is_returned: bool,
        visitor: &mut dyn FnMut(Id, &mut Expression, bool) -> VisitorResult,
    ) -> VisitorResult {
        if let Expression::Lambda { body, .. } | Expression::Multiple(body) = expression {
            match body.visit(visitor) {
                VisitorResult::Continue => {}
                VisitorResult::Abort => return VisitorResult::Abort,
            }
        }
        visitor(id, expression, is_returned)
    }

    /// Calls the visitor for each contained expression, even expressions in
    /// lambdas or multiples.
    ///
    /// The visitor is called in inside-out order, so if the body contains a
    /// lambda, the visitor is first called for its body expressions and only
    /// then for the lambda expression itself.
    ///
    /// The visitor takes the ID of the current expression as well as the
    /// expression itself. It also takes `VisibleExpressions`, which allows it
    /// to inspect all expressions currently in scope. Finally, the visitor also
    /// receives whether the current expression is returned from the surrounding
    /// body.
    pub fn visit_with_visible(
        &mut self,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        self.visit_with_visible_rec(&mut VisibleExpressions::none_visible(), visitor);
    }
    fn visit_with_visible_rec(
        &mut self,
        visible: &mut VisibleExpressions,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        let expressions_in_this_body = self.expressions.iter().map(|(id, _)| *id).collect_vec();
        let length = expressions_in_this_body.len();

        for index in 0..length {
            let (id, mut expression) = mem::replace(
                self.expressions.get_mut(index).unwrap(),
                (Id::from_usize(0), Expression::Parameter),
            );
            let is_returned = index == length - 1;
            Self::visit_expression_with_visible(id, &mut expression, visible, is_returned, visitor);
            visible.insert(id, expression);
        }

        for (index, id) in expressions_in_this_body.iter().enumerate() {
            *self.expressions.get_mut(index).unwrap() =
                (*id, visible.expressions.remove(id).unwrap());
        }
    }
    fn visit_expression_with_visible(
        id: Id,
        expression: &mut Expression,
        visible: &mut VisibleExpressions,
        is_returned: bool,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        if let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            ..
        } = expression
        {
            for parameter in parameters.iter() {
                visible.insert(*parameter, Expression::Parameter);
            }
            visible.insert(*responsible_parameter, Expression::Parameter);
            body.visit_with_visible_rec(visible, visitor);
            for parameter in parameters.iter() {
                visible.expressions.remove(parameter);
            }
            visible.expressions.remove(responsible_parameter);
        }
        if let Expression::Multiple(body) = expression {
            body.visit_with_visible_rec(visible, visitor);
        }

        visitor(id, expression, visible, is_returned);
    }

    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        for (_, expression) in self.iter_mut() {
            expression.visit_bodies(visitor);
        }
        visitor(self);
    }
}
impl Expression {
    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        match self {
            Expression::Lambda { body, .. } => body.visit_bodies(visitor),
            Expression::Multiple(body) => body.visit_bodies(visitor),
            _ => {}
        }
    }
}

#[derive(Deref, DerefMut)]
pub struct LambdaBodyBuilder {
    #[deref]
    #[deref_mut]
    body_builder: BodyBuilder,
    responsible_parameter: Id,
    parameters: Vec<Id>,
}
impl LambdaBodyBuilder {
    fn new(mut id_generator: IdGenerator<Id>) -> Self {
        let responsible_parameter = id_generator.generate();
        LambdaBodyBuilder {
            body_builder: BodyBuilder::new(id_generator),
            parameters: vec![],
            responsible_parameter,
        }
    }

    pub fn new_parameter(&mut self) -> Id {
        let id = self.body_builder.id_generator.generate();
        self.parameters.push(id);
        id
    }

    fn finish(self) -> (IdGenerator<Id>, Expression) {
        let (id_generator, body) = self.body_builder.finish();
        let lambda = Expression::Lambda {
            parameters: self.parameters,
            responsible_parameter: self.responsible_parameter,
            body,
        };
        (id_generator, lambda)
    }
}

pub struct BodyBuilder {
    id_generator: IdGenerator<Id>,
    body: Body,
}
impl BodyBuilder {
    pub fn new(id_generator: IdGenerator<Id>) -> Self {
        BodyBuilder {
            id_generator,
            body: Body::default(),
        }
    }

    pub fn push(&mut self, expression: Expression) -> Id {
        self.body
            .push_with_new_id(&mut self.id_generator, expression)
    }
    #[cfg(test)]
    pub fn push_multiple<F>(&mut self, function: F) -> Id
    where
        F: FnOnce(&mut BodyBuilder),
    {
        let mut body = BodyBuilder::new(mem::take(&mut self.id_generator));
        function(&mut body);
        let (id_generator, body) = body.finish();
        self.id_generator = id_generator;
        self.push(Expression::Multiple(body))
    }

    pub fn push_int(&mut self, value: BigInt) -> Id {
        self.push(Expression::Int(value))
    }
    pub fn push_text(&mut self, value: String) -> Id {
        self.push(Expression::Text(value))
    }

    pub fn push_symbol(&mut self, value: String) -> Id {
        self.push(Expression::Symbol(value))
    }
    pub fn push_nothing(&mut self) -> Id {
        self.push(Expression::nothing())
    }
    pub fn push_bool(&mut self, value: bool) -> Id {
        self.push(value.into())
    }

    pub fn push_builtin(&mut self, function: BuiltinFunction) -> Id {
        self.push(Expression::Builtin(function))
    }
    pub fn push_list(&mut self, list: Vec<Id>) -> Id {
        self.push(Expression::List(list))
    }
    pub fn push_struct(&mut self, struct_: Vec<(Id, Id)>) -> Id {
        self.push(Expression::Struct(struct_))
    }
    pub fn push_reference(&mut self, reference: Id) -> Id {
        self.push(Expression::Reference(reference))
    }
    pub fn push_hir_id(&mut self, id: hir::Id) -> Id {
        self.push(Expression::HirId(id))
    }

    /// The builder function takes the builder and the responsible parameter.
    pub fn push_lambda<F>(&mut self, function: F) -> Id
    where
        F: FnOnce(&mut LambdaBodyBuilder, Id),
    {
        let mut builder = LambdaBodyBuilder::new(mem::take(&mut self.id_generator));
        let responsible_parameter = builder.responsible_parameter;
        function(&mut builder, responsible_parameter);
        let (id_generator, lambda) = builder.finish();
        self.id_generator = id_generator;
        self.push(lambda)
    }

    pub fn push_call(&mut self, function: Id, arguments: Vec<Id>, responsible: Id) -> Id {
        self.push(Expression::Call {
            function,
            arguments,
            responsible,
        })
    }
    pub fn push_if_else<T, E>(
        &mut self,
        condition: Id,
        then_builder: T,
        else_builder: E,
        responsible: Id,
    ) -> Id
    where
        T: FnOnce(&mut Self),
        E: FnOnce(&mut Self),
    {
        let builtin_if_else = self.push_builtin(BuiltinFunction::IfElse);
        let then_lambda = self.push_lambda(|body, _| then_builder(body));
        let else_lambda = self.push_lambda(|body, _| else_builder(body));
        self.push_call(
            builtin_if_else,
            vec![condition, then_lambda, else_lambda],
            responsible,
        )
    }

    pub fn push_panic(&mut self, reason: Id, responsible: Id) -> Id {
        self.push(Expression::Panic {
            reason,
            responsible,
        })
    }

    pub fn current_return_value(&mut self) -> Id {
        self.body.return_value()
    }

    pub fn finish(self) -> (IdGenerator<Id>, Body) {
        (self.id_generator, self.body)
    }
}

impl ToRichIr<MirReferenceKey> for Body {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder<MirReferenceKey>) {
        fn push(builder: &mut RichIrBuilder<MirReferenceKey>, id: &Id, expression: &Expression) {
            let range = builder.push(
                id.to_short_debug_string(),
                Some(TokenType::Variable),
                EnumSet::empty(),
            );
            builder.push_definition(id.to_owned(), range);

            builder.push(" = ", None, EnumSet::empty());
            expression.build_rich_ir(builder);
        }

        let mut iterator = self.expressions.iter();
        if let Some((id, expression)) = iterator.next() {
            push(builder, id, expression);
        }
        for (id, expression) in iterator {
            builder.push_newline();
            push(builder, id, expression);
        }
    }
}

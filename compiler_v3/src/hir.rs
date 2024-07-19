use crate::{id::CountableId, impl_countable_id};
use derive_more::Deref;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};
use strum::{AsRefStr, VariantArray};

#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(usize);
impl_countable_id!(Id);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hir {
    pub assignments: Vec<(Id, Box<str>, Definition)>,
}
impl Hir {
    #[must_use]
    pub fn type_of(&self, id: Id) -> Cow<Type> {
        self.assignments
            .iter()
            .find_map(|(i, _, definition)| {
                if i != &id {
                    return None;
                }

                Some(match definition {
                    Definition::Value { type_, .. } => Cow::Borrowed(type_),
                    Definition::Function {
                        parameters,
                        return_type,
                        ..
                    } => Cow::Owned(Type::Function {
                        parameter_types: parameters
                            .iter()
                            .map(|parameter| parameter.type_.clone())
                            .collect(),
                        return_type: Box::new(return_type.clone()),
                    }),
                })
            })
            .unwrap_or_else(|| {
                self.assignments
                    .iter()
                    .find_map(|(_, _, definition)| definition.type_of(id))
                    .unwrap()
            })
    }
    // /// `None` means the ID belongs to a parameter.
    // #[must_use]
    // pub fn get(&self, id: Id) -> Option<&Assignment> {
    //     self.assignments.iter().find_map(|(i, _, assignment)| {
    //         if i == &id {
    //             Some(assignment)
    //         } else {
    //             assignment.get(id)
    //         }
    //     })
    // }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Definition {
    Value {
        type_: Type,
        value: Expression,
    },
    Function {
        parameters: Box<[Parameter]>,
        return_type: Type,
        body: BodyOrBuiltin,
    },
}
impl Definition {
    #[must_use]
    pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
        match self {
            Definition::Value { value, .. } => value.type_of(id),
            Definition::Function { body, .. } => match body {
                BodyOrBuiltin::Body(body) => body.type_of(id),
                BodyOrBuiltin::Builtin(_) => None,
            },
        }
    }
    // #[must_use]
    // pub fn get(&self, id: Id) -> Option<&Assignment> {
    //     match self {
    //         Self::Value { value,.. } => value.get(id),
    //         Self::Function { body, .. } => body.get(id),
    //     }
    // }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Type,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum BodyOrBuiltin {
    Body(Body),
    Builtin(BuiltinFunction),
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Id, Option<Box<str>>, Expression, Type)>,
}
impl Body {
    #[must_use]
    pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
        self.expressions
            .iter()
            .find_map(|(i, _, expression, type_)| {
                if i == &id {
                    Some(Cow::Borrowed(type_))
                } else {
                    expression.type_of(id)
                }
            })
    }
    // #[must_use]
    // pub fn get(&self, id: Id) -> Option<&Assignment> {
    //     match self {
    //         Body::Builtin(_) => None,
    //         Body::Written { expressions } => {
    //             expressions.iter().find_map(|(i, _, expression, _)| {
    //                 if i == &id {
    //                     Some(expression)
    //                 } else {
    //                     expression.get(id)
    //                 }
    //             })
    //         }
    //     }
    // }

    fn collect_defined_and_referenced_ids(
        &self,
        defined_ids: &mut FxHashSet<Id>,
        referenced_ids: &mut FxHashSet<Id>,
    ) {
        for (id, _, expression, _) in &self.expressions {
            defined_ids.insert(*id);
            match expression {
                Expression::Int(_)
                | Expression::Text(_)
                | Expression::Tag { .. }
                | Expression::Struct(_)
                | Expression::StructAccess { .. }
                | Expression::ValueWithTypeAnnotation { .. }
                | Expression::Reference(_)
                | Expression::Call { .. }
                | Expression::Or { .. }
                | Expression::CreateOrVariant { .. }
                | Expression::Type(_)
                | Expression::Error => {}
                Expression::Lambda(Lambda { parameters, body }) => {
                    defined_ids.extend(parameters.iter().map(|it| it.id));
                    body.collect_defined_and_referenced_ids(defined_ids, referenced_ids);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Expression {
    Int(i64),
    Text(Box<str>),
    Tag {
        symbol: Box<str>,
        value: Option<Box<Expression>>,
    },
    Struct(Box<[(Box<str>, Expression)]>),
    StructAccess {
        struct_: Box<Expression>,
        field: Box<str>,
    },
    ValueWithTypeAnnotation {
        value: Box<Expression>,
        type_: Type,
    },
    Lambda(Lambda),
    Reference(Id),
    Call {
        receiver: Box<Expression>,
        arguments: Box<[Expression]>,
    },
    Or {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    CreateOrVariant {
        or_type: OrType,
        symbol: Box<str>,
        value: Box<Expression>,
    },
    Type(Type),
    Error,
}
impl Expression {
    #[must_use]
    pub fn nothing() -> Self {
        Self::Tag {
            symbol: Box::from("Nothing"),
            value: None,
        }
    }
}
impl Expression {
    #[must_use]
    pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Tag { .. }
            | Expression::Struct(_)
            | Expression::StructAccess { .. }
            | Expression::ValueWithTypeAnnotation { .. }
            | Expression::Reference(_)
            | Expression::Call { .. }
            | Expression::Or { .. }
            | Expression::CreateOrVariant { .. }
            | Expression::Type(_)
            | Expression::Error => None,
            Expression::Lambda(Lambda { parameters, body }) => {
                if let Some(parameter) = parameters.iter().find(|it| it.id == id) {
                    return Some(Cow::Borrowed(&parameter.type_));
                }

                body.type_of(id)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Lambda {
    pub parameters: Box<[Parameter]>,
    pub body: Body,
}
impl Lambda {
    #[must_use]
    pub fn closure(&self) -> FxHashSet<Id> {
        let mut defined_ids = self.parameters.iter().map(|it| it.id).collect();
        let mut referenced_ids = FxHashSet::default();
        self.body
            .collect_defined_and_referenced_ids(&mut defined_ids, &mut referenced_ids);
        referenced_ids.retain(|id| !defined_ids.contains(id));
        referenced_ids
    }
}

#[derive(AsRefStr, Clone, Copy, Debug, Eq, Hash, PartialEq, VariantArray)]
#[strum(serialize_all = "camelCase")]
pub enum BuiltinFunction {
    IntAdd,
    Print,
    TextConcat,
}
impl BuiltinFunction {
    #[must_use]
    pub fn id(self) -> Id {
        Id::from_usize(self as usize)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    #[allow(clippy::enum_variant_names)]
    Type,
    Tag(TagType),
    Or(OrType),
    Int,
    Text,
    Struct(Box<[(Box<str>, Type)]>),
    Function {
        parameter_types: Box<[Type]>,
        return_type: Box<Type>,
    },
    Error,
}
impl Type {
    #[must_use]
    pub fn nothing() -> Self {
        Self::Tag(TagType::nothing())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TagType {
    pub symbol: Box<str>,
    pub value_type: Option<Box<Type>>,
}
impl TagType {
    #[must_use]
    pub fn nothing() -> Self {
        Self {
            symbol: Box::from("Nothing"),
            value_type: None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OrType(pub Box<[TagType]>);

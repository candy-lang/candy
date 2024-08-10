use crate::{id::CountableId, impl_countable_id};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::fmt::{self, Display, Formatter};
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
    pub type_declarations: FxHashMap<Box<str>, TypeDeclaration>,
    pub assignments: Box<[(Id, Box<str>, Assignment)]>,
    pub assignment_initialization_order: Vec<Id>,
    pub functions: Box<[(Id, Box<str>, Function)]>,
}
// impl Hir {
//     /// `None` means the ID belongs to a parameter.
//     #[must_use]
//     pub fn get(&self, id: Id) -> Option<&Assignment> {
//         self.assignments.iter().find_map(|(i, _, assignment)| {
//             if i == &id {
//                 Some(assignment)
//             } else {
//                 assignment.get(id)
//             }
//         })
//     }
// }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeDeclaration {
    Struct {
        fields: Box<[(Box<str>, Type)]>,
    },
    Enum {
        variants: Box<[(Box<str>, Option<Type>)]>,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Named(Box<str>),
    // TODO: `Self` type
    Error,
}
impl Type {
    #[must_use]
    pub fn nothing() -> Self {
        Self::Named("Nothing".into())
    }
    #[must_use]
    pub fn never() -> Self {
        Self::Named("Never".into())
    }

    #[must_use]
    pub fn int() -> Self {
        Self::Named("Int".into())
    }
    #[must_use]
    pub fn text() -> Self {
        Self::Named("Text".into())
    }
}
impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self {
            Type::Named(name) => write!(f, "{name}"),
            Type::Error => write!(f, "<error>"),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Assignment {
    pub type_: Type,
    pub body: Body,
}
// impl Definition {
//     #[must_use]
//     pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
//         match self {
//             Definition::Value { value, .. } => value.type_of(id),
//             Definition::Function { body, .. } => match body {
//                 BodyOrBuiltin::Body(body) => body.type_of(id),
//                 BodyOrBuiltin::Builtin(_) => None,
//             },
//         }
//     }
//     // #[must_use]
//     // pub fn get(&self, id: Id) -> Option<&Assignment> {
//     //     match self {
//     //         Self::Value { value,.. } => value.get(id),
//     //         Self::Function { body, .. } => body.get(id),
//     //     }
//     // }
// }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Function {
    pub parameters: Box<[Parameter]>,
    pub return_type: Type,
    pub body: BodyOrBuiltin,
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
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Body {
    pub expressions: Vec<(Id, Option<Box<str>>, Expression)>,
}
impl Body {
    pub fn return_value_id(&self) -> Id {
        self.expressions.last().unwrap().0
    }
    // #[must_use]
    // pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
    //     self.expressions
    //         .iter()
    //         .find_map(|(i, _, expression, type_)| {
    //             if i == &id {
    //                 Some(Cow::Borrowed(type_))
    //             } else {
    //                 expression.type_of(id)
    //             }
    //         })
    // }
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

    // fn collect_defined_and_referenced_ids(
    //     &self,
    //     defined_ids: &mut FxHashSet<Id>,
    //     referenced_ids: &mut FxHashSet<Id>,
    // ) {
    //     for (id, _, expression, _) in &self.expressions {
    //         defined_ids.insert(*id);
    //         match expression {
    //             Expression::Int(_)
    //             | Expression::Text(_)
    //             | Expression::Tag { .. }
    //             | Expression::Struct(_)
    //             | Expression::StructAccess { .. }
    //             | Expression::ValueWithTypeAnnotation { .. }
    //             | Expression::Reference(_)
    //             | Expression::Call { .. }
    //             | Expression::Or { .. }
    //             | Expression::CreateOrVariant { .. }
    //             | Expression::Type(_)
    //             | Expression::Error => {}
    //             Expression::Lambda(Lambda { parameters, body }) => {
    //                 defined_ids.extend(parameters.iter().map(|it| it.id));
    //                 body.collect_defined_and_referenced_ids(defined_ids, referenced_ids);
    //             }
    //         }
    //     }
    // }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub type_: Type,
}
impl Expression {
    #[must_use]
    pub const fn error() -> Self {
        Self {
            kind: ExpressionKind::Error,
            type_: Type::Error,
        }
    }
    #[must_use]
    pub fn nothing() -> Self {
        Self {
            kind: ExpressionKind::CreateStruct {
                struct_: Type::nothing(),
                fields: [].into(),
            },
            type_: Type::nothing(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ExpressionKind {
    Int(i64),
    Text(Box<str>),
    CreateStruct {
        struct_: Type,
        fields: Box<[Id]>,
    },
    StructAccess {
        struct_: Id,
        field: Box<str>,
    },
    CreateEnum {
        enum_: Type,
        variant: Box<str>,
        value: Option<Id>,
    },
    Reference(Id),
    Call {
        function: Id,
        arguments: Box<[Id]>,
    },
    Switch {
        value: Id,
        enum_: Type,
        cases: Box<[(Box<str>, Option<Id>, Body)]>,
    },
    Error,
}
// impl Expression {
//     #[must_use]
//     pub fn type_of(&self, id: Id) -> Option<Cow<Type>> {
//         match self {
//             Expression::Int(_)
//             | Expression::Text(_)
//             | Expression::Tag { .. }
//             | Expression::Struct(_)
//             | Expression::StructAccess { .. }
//             | Expression::ValueWithTypeAnnotation { .. }
//             | Expression::Reference(_)
//             | Expression::Call { .. }
//             | Expression::Or { .. }
//             | Expression::CreateOrVariant { .. }
//             | Expression::Type(_)
//             | Expression::Error => None,
//             Expression::Lambda(Lambda { parameters, body }) => {
//                 if let Some(parameter) = parameters.iter().find(|it| it.id == id) {
//                     return Some(Cow::Borrowed(&parameter.type_));
//                 }

//                 body.type_of(id)
//             }
//         }
//     }
// }

#[derive(AsRefStr, Clone, Copy, Debug, Eq, Hash, PartialEq, VariantArray)]
#[strum(serialize_all = "camelCase")]
pub enum BuiltinFunction {
    IntAdd,
    IntCompareTo,
    IntSubtract,
    IntToText,
    Print,
    TextConcat,
}
impl BuiltinFunction {
    #[must_use]
    pub fn id(self) -> Id {
        Id::from_usize(self as usize)
    }
}

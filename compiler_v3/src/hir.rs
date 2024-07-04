use crate::{id::CountableId, impl_countable_id};
use derive_more::Deref;
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
    pub assignments: Vec<(Id, Box<str>, Definition)>,
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
pub enum Definition {
    Value {
        type_: Type,
        value: Expression,
    },
    Function {
        parameters: Box<[Parameter]>,
        return_type: Type,
        body: Body,
    },
}
// impl Assignment {
//     #[must_use]
//     pub fn get(&self, id: Id) -> Option<&Assignment> {
//         match self {
//             Self::Value { value,.. } => value.get(id),
//             Self::Function { body, .. } => body.get(id),
//         }
//     }
// }
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Type,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Body {
    Builtin(BuiltinFunction),
    Written {
        expressions: Vec<(Id, Option<Box<str>>, Expression, Type)>,
    },
}
// impl Body {
//     #[must_use]
//     pub fn get(&self, id: Id) -> Option<&Assignment> {
//         match self {
//             Body::Builtin(_) => None,
//             Body::Written { expressions } => {
//                 expressions.iter().find_map(|(i, _, expression, _)| {
//                     if i == &id {
//                         Some(expression)
//                     } else {
//                         expression.get(id)
//                     }
//                 })
//             }
//         }
//     }
// }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Expression {
    Symbol(Box<str>),
    Int(i64),
    Text(Box<str>),
    Struct(Box<[(Box<str>, Expression)]>),
    StructAccess {
        struct_: Box<Expression>,
        field: Box<str>,
    },
    ValueWithTypeAnnotation {
        value: Box<Expression>,
        type_: Type,
    },
    Lambda {
        parameters: Box<[Parameter]>,
        body: Body,
    },
    Reference(Id),
    Call {
        receiver: Box<Expression>,
        arguments: Box<[Expression]>,
    },
    Type(Type),
    Error,
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
    Type,
    Symbol, // TODO: specific symbol as separate type
    Int,
    Text,
    Struct(Box<[(Box<str>, Type)]>),
    Function {
        parameter_types: Box<[Type]>,
        return_type: Box<Type>,
    },
    Error,
}

use crate::{id::CountableId, impl_countable_id};
use derive_more::Deref;
use std::fmt::{self, Display, Formatter};
use strum::{AsRefStr, VariantArray};

#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, PartialEq)]
pub struct Id(usize);
impl_countable_id!(Id);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hir {
    pub assignments: Vec<(Id, Box<str>, Assignment)>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Assignment {
    Value {
        value: Expression,
        type_: Type,
    },
    Function {
        parameters: Box<[Parameter]>,
        return_type: Type,
        body: Body,
    },
}
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

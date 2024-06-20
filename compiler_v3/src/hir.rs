use crate::{ast::AstString, impl_countable_id};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(pub usize);
impl_countable_id!(Id);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Hir {
    pub assignments: Vec<(Id, Box<str>, Assignment)>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Assignment {
    Value {
        type_: Body,
        value: Body,
    },
    Function {
        parameters: Box<[Parameter]>,
        return_type: Body,
        body: Body,
    },
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Body,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Body {
    pub identifiers: Vec<(Id, AstString)>,
    pub expressions: Vec<(Id, Expression)>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Expression {
    Symbol(Box<str>),
    Int(i64),
    Text(String),
    Struct(Box<[(Box<str>, Id)]>),
    StructAccess { struct_: Id, field: Box<str> },
    ValueWithTypeAnnotation { value: Id, type_: Id },
    IntType,
    TextType,
    Reference(Id),
    Call(Id, Box<[Id]>),
    BuiltinEquals,
    BuiltinPrint,
    Error,
}

use crate::{hir::BuiltinFunction, impl_countable_id};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Default, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id(usize);
impl_countable_id!(Id);
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "_{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mono {
    pub type_declarations: FxHashMap<Box<str>, TypeDeclaration>,
    pub assignments: FxHashMap<Box<str>, Assignment>,
    pub assignment_initialization_order: Box<[Box<str>]>,
    pub functions: FxHashMap<Box<str>, Function>,
    pub main_function: Box<str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeDeclaration {
    Struct {
        fields: Box<[(Box<str>, Box<str>)]>,
    },
    Enum {
        variants: Box<[(Box<str>, Option<Box<str>>)]>,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Assignment {
    pub type_: Box<str>,
    pub body: Body,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Function {
    pub parameters: Box<[Parameter]>,
    pub return_type: Box<str>,
    pub body: BodyOrBuiltin,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Parameter {
    pub id: Id,
    pub name: Box<str>,
    pub type_: Box<str>,
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
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub type_: Box<str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ExpressionKind {
    Int(i64),
    Text(Box<str>),
    CreateStruct {
        struct_: Box<str>,
        fields: Box<[Id]>,
    },
    StructAccess {
        struct_: Id,
        field: Box<str>,
    },
    CreateEnum {
        enum_: Box<str>,
        variant: Box<str>,
        value: Option<Id>,
    },
    GlobalAssignmentReference(Box<str>),
    LocalReference(Id),
    Call {
        function: Box<str>,
        arguments: Box<[Id]>,
    },
    Switch {
        value: Id,
        enum_: Box<str>,
        cases: Box<[SwitchCase]>,
    },
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SwitchCase {
    pub variant: Box<str>,
    pub value_id: Option<Id>,
    pub body: Body,
}

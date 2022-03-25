use std::ops::Deref;

use linked_hash_map::LinkedHashMap;

use crate::input::Input;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Id {
    pub input: Input,
    pub local: usize,
}
impl Id {
    pub fn new(input: Input, local: usize) -> Self {
        Self { input, local }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Ast {
    pub id: Id,
    pub kind: AstKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AstKind {
    Int(Int),
    Text(Text),
    Identifier(Identifier),
    Symbol(Symbol),
    Struct(Struct),
    Lambda(Lambda),
    Call(Call),
    Assignment(Assignment),
    Error,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Int(pub u64);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Text(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Identifier(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Struct {
    pub entries: LinkedHashMap<Ast, Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Lambda {
    pub parameters: Vec<AstString>,
    pub body: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Call {
    pub name: AstString,
    pub arguments: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assignment {
    pub name: AstString,
    pub body: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct AstString {
    pub id: Id,
    pub value: String,
}
impl Deref for AstString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

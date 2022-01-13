#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Ast {
    Int(Int),
    Text(Text),
    Symbol(Symbol),
    Lambda(Lambda),
    Call(Call),
    Assignment(Assignment),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Int(pub u64);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Text(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Lambda {
    pub parameters: Vec<String>,
    pub body: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Call {
    pub name: String,
    pub arguments: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assignment {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Ast>,
}

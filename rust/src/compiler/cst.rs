#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Cst {
    Int(Int),
    String(String),
    Symbol(Symbol),
    Call(Call),
    Assignment(Assignment),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Int(pub u64);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Call {
    pub name: String,
    pub arguments: Vec<Cst>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assignment {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Cst>,
}

use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash)]
pub enum BuiltinFunction {
    Add,
    Equals,
    GetArgumentCount,
    IfElse,
    Panic,
    Print,
    TypeOf,
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

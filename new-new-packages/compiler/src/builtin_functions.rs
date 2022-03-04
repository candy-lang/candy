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
    StructGet,     // struct.get struct key -> value
    StructGetKeys, // struct.getKeys struct -> listOfKeys
    StructHasKey,  // struct.hasKey struct key -> bool
    TypeOf,
    // TODO: add some way of getting keys and values from a struct
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

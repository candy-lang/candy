use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash, Copy)]
pub enum BuiltinFunction {
    Add,              // int int -> int
    Equals,           // any any -> booleanSymbol
    GetArgumentCount, // closure -> argumentCount
    IfElse,           // condition thenClosure elseClosure -> resultOfExecutedClosure
    Panic,            // message -> Never
    Print,            // message -> Nothing
    StructGet,        // struct key -> value
    StructGetKeys,    // struct -> listOfKeys
    StructHasKey,     // struct key -> bool
    TypeOf,           // any -> typeSymbol
    UseAsset,         // currentPath target -> targetAsString
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

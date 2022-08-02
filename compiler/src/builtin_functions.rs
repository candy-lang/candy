use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash, Copy)]
pub enum BuiltinFunction {
    Add,              // (summandA: int) (summandB: int) -> (sum: int)
    Equals,           // (a: any) (b: any) -> (boolean: True | False)
    GetArgumentCount, // (closure: Lambda) -> (argumentCount: int)
    IfElse, // (condition: True | False) (then: lambda) (else: lambda) -> (resultOfExecutedClosure: any)
    Print,  // (message: Text) -> Nothing
    StructGet, // (struct: struct) (key: any) -> (value: any)
    StructGetKeys, // (struct: struct) -> (listOfKeys: listOfAny)
    StructHasKey, // (struct: struct) (key: any) -> (isKeyInStruct: True | False)
    TypeOf, // (value: any) -> (type: Int | Text | Symbol | Struct | Function | Builtin)
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

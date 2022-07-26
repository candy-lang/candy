use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash, Copy)]
pub enum BuiltinFunction {
    Call,                // (lambdaWith0Arguments) -> (returnValue: any)
    Equals,              // any any -> booleanSymbol
    GetArgumentCount,    // closure -> argumentCount
    IfElse,              // condition thenClosure elseClosure -> resultOfExecutedClosure
    IntAdd,              // (summandA: int) (summandB: int) -> (sum: int)
    IntCompareTo,        // (valueA: int) (valueB: int) -> (ordering: Less | Equal | Greater)
    IntDivideTruncating, // (dividend: int) (divisor: int) -> (quotient: int)
    IntModulo,           // (dividend: int) (divisor: int) -> (remainder: int)
    IntMultiply,         // (factorA: int) (factorB: int) -> (product: int)
    IntParse,            // (text: text) -> (parsedInt: maybeOfInt)
    IntSubtract,         // (minuend: int) (subtrahend: int) -> (difference: int)
    Panic,               // message -> Never
    Print,               // message -> Nothing
    StructGet,           // struct key -> value
    StructGetKeys,       // struct -> listOfKeys
    StructHasKey,        // struct key -> bool
    TextConcatenate,     // (valueA: text) (valueB: text) -> (concatenated: text)
    TypeOf,              // any -> typeSymbol
    UseAsset,            // currentPath target -> targetAsString
    UseLocalModule,      // currentPath target -> targetAsStruct
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

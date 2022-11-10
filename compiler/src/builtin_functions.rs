use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash, Copy)]
pub enum BuiltinFunction {
    ChannelCreate,       // capacity -> [sendPort, receivePort]
    ChannelSend,         // channel any -> Nothing
    ChannelReceive,      // channel -> any
    Equals,              // any any -> booleanSymbol
    FunctionRun,         // (lambdaWith0Arguments) -> (returnValue: any)
    GetArgumentCount,    // closure -> argumentCount
    IfElse,              // condition thenClosure elseClosure -> resultOfExecutedClosure
    IntAdd,              // (summandA: int) (summandB: int) -> (sum: int)
    IntBitLength,        // (value: int) -> (numberOfBits: int)
    IntBitwiseAnd,       // (valueA: int) (valueB: int) -> (result: int)
    IntBitwiseOr,        // (valueA: int) (valueB: int) -> (result: int)
    IntBitwiseXor,       // (valueA: int) (valueB: int) -> (result: int)
    IntCompareTo,        // (valueA: int) (valueB: int) -> (ordering: Less | Equal | Greater)
    IntDivideTruncating, // (dividend: int) (divisor: int) -> (quotient: int)
    IntModulo,           // (dividend: int) (divisor: int) -> (modulus: int)
    IntMultiply,         // (factorA: int) (factorB: int) -> (product: int)
    IntParse,            // text -> (parsedInt: maybeOfInt)
    IntRemainder,        // (dividend: int) (divisor: int) -> (remainder: int)
    IntShiftLeft,        // (value: int) (amount: int) -> (shifted: int)
    IntShiftRight,       // (value: int) (amount: int) -> (shifted: int)
    IntSubtract,         // (minuend: int) (subtrahend: int) -> (difference: int)
    ListFilled,          // (length: int) item -> list
    ListGet,             // list (index: int) -> item
    ListInsert,          // list (index: int) item -> list
    ListLength,          // list -> int
    ListRemoveAt,        // list (index: int) -> (list, item)
    ListReplace,         // list (index: int) newItem -> list
    Parallel,            // body: Closure -> returnValueOfClosure
    Print,               // message -> Nothing
    StructGet,           // struct key -> value
    StructGetKeys,       // struct -> listOfKeys
    StructHasKey,        // struct key -> booleanSymbol
    TextCharacters,      // text -> (listOfText: list)
    TextConcatenate,     // (valueA: text) (valueB: text) -> (concatenated: text)
    TextContains,        // text (pattern: text) -> booleanSymbol
    TextEndsWith,        // text (pattern: text) -> booleanSymbol
    TextGetRange,        // text (startInclusive: int) (endExclusive: int) -> (substring: text)
    TextIsEmpty,         // text -> (isEmpty: booleanSymbol)
    TextLength,          // text -> (length: int)
    TextStartsWith,      // text (pattern: text) -> booleanSymbol
    TextTrimEnd,         // text -> text
    TextTrimStart,       // text -> text
    Try,                 // closure -> okWithClosureResultOrErrorWithPanicReason
    TypeOf,              // any -> typeSymbol
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

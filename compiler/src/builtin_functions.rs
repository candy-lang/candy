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

    /// Implementation note: Why does `✨.parallel` not return the value
    /// directly? The current architecture is chosen to make the VM's code more
    /// uniform – every nursery child just sends its result to a channel and
    /// there's no special casing for the first child, the closure passed to
    /// `✨.parallel`.
    Parallel,            // (body: Closure, result: SendPort) -> Nothing
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
    TypeOf,              // any -> typeSymbol
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

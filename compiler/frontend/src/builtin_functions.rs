use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter, PartialEq, Eq, Clone, Hash, Copy)]
pub enum BuiltinFunction {
    ChannelCreate,       // capacity -> [sendPort, receivePort]
    ChannelSend,         // channel any -> Nothing
    ChannelReceive,      // channel -> any
    Equals,              // any any -> booleanTag
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
    StructHasKey,        // struct key -> booleanTag
    TagGetValue,         // tag -> any
    TagHasValue,         // tag -> booleanTag
    TagWithoutValue,     // tag -> tag
    TextCharacters,      // text -> (listOfText: list)
    TextConcatenate,     // (textA: text) (textB: text) -> (concatenated: text)
    TextContains,        // text (pattern: text) -> booleanTag
    TextEndsWith,        // text (pattern: text) -> booleanTag
    TextFromUtf8,        // (bytes: listOfInteger) -> resultOfText
    TextGetRange,        // text (startInclusive: int) (endExclusive: int) -> (substring: text)
    TextIsEmpty,         // text -> (isEmpty: booleanTag)
    TextLength,          // text -> (length: int)
    TextStartsWith,      // text (pattern: text) -> booleanTag
    TextTrimEnd,         // text -> text
    TextTrimStart,       // text -> text
    ToDebugText,         // any -> text
    Try,                 // closure -> okWithClosureResultOrErrorWithPanicReason
    TypeOf,              // any -> typeTag
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

impl BuiltinFunction {
    pub fn is_pure(&self) -> bool {
        match self {
            BuiltinFunction::ChannelCreate => false,
            BuiltinFunction::ChannelSend => false,
            BuiltinFunction::ChannelReceive => false,
            BuiltinFunction::Equals => true,
            BuiltinFunction::FunctionRun => false,
            BuiltinFunction::GetArgumentCount => true,
            BuiltinFunction::IfElse => false,
            BuiltinFunction::IntAdd => true,
            BuiltinFunction::IntBitLength => true,
            BuiltinFunction::IntBitwiseAnd => true,
            BuiltinFunction::IntBitwiseOr => true,
            BuiltinFunction::IntBitwiseXor => true,
            BuiltinFunction::IntCompareTo => true,
            BuiltinFunction::IntDivideTruncating => true,
            BuiltinFunction::IntModulo => true,
            BuiltinFunction::IntMultiply => true,
            BuiltinFunction::IntParse => true,
            BuiltinFunction::IntRemainder => true,
            BuiltinFunction::IntShiftLeft => true,
            BuiltinFunction::IntShiftRight => true,
            BuiltinFunction::IntSubtract => true,
            BuiltinFunction::ListFilled => true,
            BuiltinFunction::ListGet => true,
            BuiltinFunction::ListInsert => true,
            BuiltinFunction::ListLength => true,
            BuiltinFunction::ListRemoveAt => true,
            BuiltinFunction::ListReplace => true,
            BuiltinFunction::Parallel => false,
            BuiltinFunction::Print => false,
            BuiltinFunction::StructGet => true,
            BuiltinFunction::StructGetKeys => true,
            BuiltinFunction::StructHasKey => true,
            BuiltinFunction::TagGetValue => true,
            BuiltinFunction::TagHasValue => true,
            BuiltinFunction::TagWithoutValue => true,
            BuiltinFunction::TextCharacters => true,
            BuiltinFunction::TextConcatenate => true,
            BuiltinFunction::TextContains => true,
            BuiltinFunction::TextEndsWith => true,
            BuiltinFunction::TextFromUtf8 => true,
            BuiltinFunction::TextGetRange => true,
            BuiltinFunction::TextIsEmpty => true,
            BuiltinFunction::TextLength => true,
            BuiltinFunction::TextStartsWith => true,
            BuiltinFunction::TextTrimEnd => true,
            BuiltinFunction::TextTrimStart => true,
            BuiltinFunction::ToDebugText => true,
            BuiltinFunction::Try => false,
            BuiltinFunction::TypeOf => true,
        }
    }

    pub fn num_parameters(&self) -> usize {
        match self {
            BuiltinFunction::ChannelCreate => 1,
            BuiltinFunction::ChannelSend => 2,
            BuiltinFunction::ChannelReceive => 1,
            BuiltinFunction::Equals => 2,
            BuiltinFunction::FunctionRun => 1,
            BuiltinFunction::GetArgumentCount => 1,
            BuiltinFunction::IfElse => 3,
            BuiltinFunction::IntAdd => 2,
            BuiltinFunction::IntBitLength => 1,
            BuiltinFunction::IntBitwiseAnd => 2,
            BuiltinFunction::IntBitwiseOr => 2,
            BuiltinFunction::IntBitwiseXor => 2,
            BuiltinFunction::IntCompareTo => 2,
            BuiltinFunction::IntDivideTruncating => 2,
            BuiltinFunction::IntModulo => 2,
            BuiltinFunction::IntMultiply => 2,
            BuiltinFunction::IntParse => 1,
            BuiltinFunction::IntRemainder => 2,
            BuiltinFunction::IntShiftLeft => 2,
            BuiltinFunction::IntShiftRight => 2,
            BuiltinFunction::IntSubtract => 2,
            BuiltinFunction::ListFilled => 2,
            BuiltinFunction::ListGet => 2,
            BuiltinFunction::ListInsert => 2,
            BuiltinFunction::ListLength => 1,
            BuiltinFunction::ListRemoveAt => 2,
            BuiltinFunction::ListReplace => 3,
            BuiltinFunction::Parallel => 1,
            BuiltinFunction::Print => 1,
            BuiltinFunction::StructGet => 2,
            BuiltinFunction::StructGetKeys => 1,
            BuiltinFunction::StructHasKey => 2,
            BuiltinFunction::TagGetValue => 1,
            BuiltinFunction::TagHasValue => 1,
            BuiltinFunction::TagWithoutValue => 1,
            BuiltinFunction::TextCharacters => 1,
            BuiltinFunction::TextConcatenate => 2,
            BuiltinFunction::TextContains => 2,
            BuiltinFunction::TextEndsWith => 2,
            BuiltinFunction::TextFromUtf8 => 1,
            BuiltinFunction::TextGetRange => 3,
            BuiltinFunction::TextIsEmpty => 1,
            BuiltinFunction::TextLength => 1,
            BuiltinFunction::TextStartsWith => 2,
            BuiltinFunction::TextTrimEnd => 1,
            BuiltinFunction::TextTrimStart => 1,
            BuiltinFunction::ToDebugText => 1,
            BuiltinFunction::Try => 1,
            BuiltinFunction::TypeOf => 1,
        }
    }
}

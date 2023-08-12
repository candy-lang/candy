use crate::{
    impl_display_via_richir,
    rich_ir::{RichIrBuilder, ToRichIr, TokenModifier, TokenType},
};
use enumset::EnumSet;
use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Clone, Copy, Debug, EnumIter, Eq, Hash, PartialEq)]
pub enum BuiltinFunction {
    Equals,              // any any -> booleanTag
    FunctionRun,         // (functionWith0Arguments) -> (returnValue: any)
    GetArgumentCount,    // function -> argumentCount
    IfElse,              // condition thenFunction elseFunction -> resultOfExecutedFunction
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
    TextEndsWith,        // text (suffix: text) -> booleanTag
    TextFromUtf8,        // (bytes: listOfInteger) -> resultOfText
    TextGetRange,        // text (startInclusive: int) (endExclusive: int) -> (substring: text)
    TextIsEmpty,         // text -> (isEmpty: booleanTag)
    TextLength,          // text -> (length: int)
    TextStartsWith,      // text (prefix: text) -> booleanTag
    TextTrimEnd,         // text -> text
    TextTrimStart,       // text -> text
    ToDebugText,         // any -> text
    TypeOf,              // any -> typeTag
}
lazy_static! {
    pub static ref VALUES: Vec<BuiltinFunction> = BuiltinFunction::iter().collect();
}

impl BuiltinFunction {
    #[must_use]
    pub const fn is_pure(&self) -> bool {
        match self {
            Self::Equals => true,
            Self::FunctionRun => false,
            Self::GetArgumentCount => true,
            Self::IfElse => false,
            Self::IntAdd => true,
            Self::IntBitLength => true,
            Self::IntBitwiseAnd => true,
            Self::IntBitwiseOr => true,
            Self::IntBitwiseXor => true,
            Self::IntCompareTo => true,
            Self::IntDivideTruncating => true,
            Self::IntModulo => true,
            Self::IntMultiply => true,
            Self::IntParse => true,
            Self::IntRemainder => true,
            Self::IntShiftLeft => true,
            Self::IntShiftRight => true,
            Self::IntSubtract => true,
            Self::ListFilled => true,
            Self::ListGet => true,
            Self::ListInsert => true,
            Self::ListLength => true,
            Self::ListRemoveAt => true,
            Self::ListReplace => true,
            Self::Print => false,
            Self::StructGet => true,
            Self::StructGetKeys => true,
            Self::StructHasKey => true,
            Self::TagGetValue => true,
            Self::TagHasValue => true,
            Self::TagWithoutValue => true,
            Self::TextCharacters => true,
            Self::TextConcatenate => true,
            Self::TextContains => true,
            Self::TextEndsWith => true,
            Self::TextFromUtf8 => true,
            Self::TextGetRange => true,
            Self::TextIsEmpty => true,
            Self::TextLength => true,
            Self::TextStartsWith => true,
            Self::TextTrimEnd => true,
            Self::TextTrimStart => true,
            Self::ToDebugText => true,
            Self::TypeOf => true,
        }
    }

    #[must_use]
    pub const fn num_parameters(&self) -> usize {
        // Responsibility parameter.
        1 + match self {
            Self::Equals => 2,
            Self::FunctionRun => 1,
            Self::GetArgumentCount => 1,
            Self::IfElse => 3,
            Self::IntAdd => 2,
            Self::IntBitLength => 1,
            Self::IntBitwiseAnd => 2,
            Self::IntBitwiseOr => 2,
            Self::IntBitwiseXor => 2,
            Self::IntCompareTo => 2,
            Self::IntDivideTruncating => 2,
            Self::IntModulo => 2,
            Self::IntMultiply => 2,
            Self::IntParse => 1,
            Self::IntRemainder => 2,
            Self::IntShiftLeft => 2,
            Self::IntShiftRight => 2,
            Self::IntSubtract => 2,
            Self::ListFilled => 2,
            Self::ListGet => 2,
            Self::ListInsert => 3,
            Self::ListLength => 1,
            Self::ListRemoveAt => 2,
            Self::ListReplace => 3,
            Self::Print => 1,
            Self::StructGet => 2,
            Self::StructGetKeys => 1,
            Self::StructHasKey => 2,
            Self::TagGetValue => 1,
            Self::TagHasValue => 1,
            Self::TagWithoutValue => 1,
            Self::TextCharacters => 1,
            Self::TextConcatenate => 2,
            Self::TextContains => 2,
            Self::TextEndsWith => 2,
            Self::TextFromUtf8 => 1,
            Self::TextGetRange => 3,
            Self::TextIsEmpty => 1,
            Self::TextLength => 1,
            Self::TextStartsWith => 2,
            Self::TextTrimEnd => 1,
            Self::TextTrimStart => 1,
            Self::ToDebugText => 1,
            Self::TypeOf => 1,
        }
    }
}

impl_display_via_richir!(BuiltinFunction);
impl ToRichIr for BuiltinFunction {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let range = builder.push(
            format!("builtin{self:?}"),
            TokenType::Function,
            EnumSet::only(TokenModifier::Builtin),
        );
        builder.push_reference(*self, range);
    }
}

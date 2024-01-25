use crate::{
    impl_display_via_richir,
    rich_ir::{RichIrBuilder, ToRichIr, TokenModifier, TokenType},
};
use enumset::EnumSet;
use lazy_static::lazy_static;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter};

/// These are all built-ins.
///
/// In the end, all Candy code boils down to some instructions. Some of those
/// instructions are grouped into `Builtins` – you can think of them as
/// functions with an implementation that's provided by the runtime.
///
/// TODO: Re-evaluate whether builtins should instead be lowered into
/// instructions directly (i.e. we would have an `IntAdd` instruction instead of
/// a builtin `IntAdd` that can be called).
///
/// Like all callable values, builtins are being passed a responsibility
/// parameter as the last argument. Because built-ins are only called through
/// corresponding functions from the `Builtins` package, all preconditions are
/// guaranteed to be true and built-ins can ignore the responsibility parameter.
///
/// See the source code of the `Builtins` package for documentation on what
/// these functions do.
#[derive(AsRefStr, Clone, Copy, Debug, EnumIter, Eq, Hash, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinFunction {
    Equals,
    FunctionRun,
    GetArgumentCount,
    IfElse,
    IntAdd,
    IntBitLength,
    IntBitwiseAnd,
    IntBitwiseOr,
    IntBitwiseXor,
    IntCompareTo,
    IntDivideTruncating,
    IntModulo,
    IntMultiply,
    IntParse,
    IntRemainder,
    IntShiftLeft,
    IntShiftRight,
    IntSubtract,
    ListFilled,
    ListGet,
    ListInsert,
    ListLength,
    ListRemoveAt,
    ListReplace,
    Print,
    StructGet,
    StructGetKeys,
    StructHasKey,
    TagGetValue,
    TagHasValue,
    TagWithoutValue,
    TagWithValue,
    TextCharacters,
    TextConcatenate,
    TextContains,
    TextEndsWith,
    TextFromUtf8,
    TextGetRange,
    TextIsEmpty,
    TextLength,
    TextStartsWith,
    TextTrimEnd,
    TextTrimStart,
    ToDebugText,
    TypeOf,
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
            Self::TagWithValue => true,
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
        match self {
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
            Self::TagWithValue => 2,
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

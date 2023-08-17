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
/// instructions directly (i.e. we would have an `IntAdd` instruction).
///
/// Like all callable values, builtins are being passed a responsibility
/// parameter as the last argument. Because built-ins are only called through
/// corresponding functions from the `Builtins` package, all preconditions are
/// guaranteed to be true and built-ins can ignore the responsibility parameter.
/// The documentation below also omits the parameter.
#[derive(AsRefStr, Clone, Copy, Debug, EnumIter, Eq, Hash, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinFunction {
    /// Takes two values. Returns `True` if both values are equal, otherwise
    /// `False`.
    ///
    /// TODO: Document how equality works. Also, change how equality for
    /// functions works.
    ///
    /// ```candy
    /// ✨.equals 5 5 => True
    /// ✨.equals 5 2 => False
    /// ```
    Equals,

    /// Takes a single function that requires zero arguments. Calls the function
    /// with the responsibility being responsible for fulfilling its needs.
    /// Returns the return value of the function.
    ///
    /// This built-in is necessary because functions in Candy are called by
    /// writing space-separated arguments such as `foo 2`. For functions that
    /// take no value, this syntax is not possible. Such functions are rare
    /// though because you don't need them if you're working with pure values.
    ///
    /// ```candy
    /// ✨.functionRun { 4 } => 4
    /// ```
    FunctionRun,

    /// Takes a single value. The value is a function, built-in, or handle.
    /// Returns the number of arguments the function requires *excluding the
    /// responsibility* argument.
    ///
    /// ```candy
    /// ✨.getArgumentCount { a -> a } => 1
    /// ✨.getArgumentCount ✨.getArgumentCount => 1
    /// ```
    GetArgumentCount,

    /// Takes a condition, a then-function, and an else-function. The condition
    /// is `True` or `False` and both given functions take zero arguments. If
    /// the condition is `True`, the then-function is executed. If it's `False`,
    /// the else-function is executed. Returns the return value of the executed
    /// function.
    ///
    /// ```candy
    /// ✨.ifElse True { 1 } { 2 } => 1
    /// ✨.ifElse False { 1 } { 2 } => 2
    /// ```
    IfElse,

    /// Takes two integers. Returns the result of adding both integers.
    ///
    /// ```candy
    /// ✨.intAdd 1 2 => 3
    /// ```
    IntAdd,

    /// Takes one integer. Returns the number of bits that are required to
    /// represent that integer, ignoring the sign.
    ///
    /// ```candy
    /// ✨.intBitLength 1 => 1
    /// ✨.intBitLength 2 => 2
    /// ✨.intBitLength 3 => 2
    /// ✨.intBitLength -3 => 2
    /// ```
    IntBitLength,

    /// Takes two integers. Returns the result of taking the bitwise "and" of
    /// both integers.
    ///
    /// ```candy
    /// ✨.intBitwiseAnd 1 2 => 1
    /// ```
    IntBitwiseAnd,

    /// Takes two integers. Returns the result of taking the bitwise "or" of
    /// both integers.
    ///
    /// ```candy
    /// ✨.intBitwiseOr 1 2 => 2
    /// ```
    IntBitwiseOr,

    /// Takes two integers. Returns the result of taking the bitwise "xor" of
    /// both integers.
    ///
    /// ```candy
    /// ✨.intBitwiseXor 1 3 => 2
    /// ```
    IntBitwiseXor,

    /// Takes two integers. Returns the relationship between the integers as a
    /// tag, which is either `Less`, `Equal`, or `Greater`.
    ///
    /// ```candy
    /// ✨.intCompareTo 1 3 => Less
    /// ✨.intCompareTo 3 3 => Equal
    /// ✨.intCompareTo 5 3 => Greater
    /// ```
    IntCompareTo,

    /// Takes two integers – a dividend and a non-zero divisor. Returns the
    /// dividend divided by the dividend.
    ///
    /// ```candy
    /// ✨.intDivideTruncating 6 3 => 2
    /// ✨.intDivideTruncating 5 3 => 1
    /// ```
    IntDivideTruncating,

    /// Takes two integers – a value and a non-zero divisor. Returns the value
    /// modulo the divisor.
    ///
    /// ```candy
    /// ✨.intModulo 6 2 => 0
    /// ✨.intModulo 5 2 => 1
    /// ```
    IntModulo,

    /// Takes two integers. Returns the result of multiplying both integers.
    ///
    /// ```candy
    /// ✨.intMultiply 6 2 => 12
    /// ```
    IntMultiply,

    /// Takes a text. If the text is the textual representation of an integer,
    /// returns `Ok` and the parsed integer. Otherwise, returns
    /// `Error NotAnInteger`.
    ///
    /// ```candy
    /// ✨.intParse "6" => Ok 6
    /// ✨.intParse "-2" => Ok -2
    /// ✨.intParse "Foo" => Error NotAnInteger
    /// ```
    IntParse,

    /// Takes two integers – a dividend and a non-zero divisor. Returns the
    /// remainder you get when dividing the dividend by the divisor.
    ///
    /// ```candy
    /// ✨.intRemainder 12 6 => 0
    /// ✨.intRemainder 13 6 => 1
    /// ```
    IntRemainder,

    /// Takes two integers – a value and a non-negative amount. Returns the
    /// value shifted to the left by the amount.
    ///
    /// ```candy
    /// ✨.intShiftLeft 1 3 => 8
    /// ```
    IntShiftLeft,

    /// Takes two integers – a value and a non-negative amount. Returns the
    /// value shifted to the right by the amount.
    ///
    /// ```candy
    /// ✨.intShiftLeft 8 3 => 1
    /// ✨.intShiftLeft 10 3 => 1
    /// ```
    IntShiftRight,

    /// Takes two integers. Returns the first minus the second.
    ///
    /// ```candy
    /// ✨.intSubtract 3 2 => 1
    /// ```
    IntSubtract,

    /// Takes an integer (the length of the list) and a value (the item).
    /// Returns a list that contains the item the given number of times.
    ///
    /// ```candy
    /// ✨.listFilled 3 Foo => (Foo, Foo, Foo)
    /// ✨.listFilled 10 1 => (1, 1, 1, 1, 1, 1, 1, 1, 1, 1)
    /// ✨.listFilled 0 Bar => (,)
    /// ```
    ListFilled,

    /// Takes a list and an integer index. The index is valid
    /// (0 <= index < length of the list). Returns the item that's at the index
    /// in the list.
    ///
    /// ```candy
    /// ✨.listGet (Foo, Bar, Baz) 0 => Foo
    /// ✨.listGet (Foo, Bar, Baz) 1 => Bar
    /// ✨.listGet (Foo, Bar, Baz) 2 => Baz
    /// ```
    ListGet,

    /// Takes a list, an integer index, and an item. The index is valid, but can
    /// point one past the end of the list (0 <= index <= length of the list).
    /// Returns a new list where the item is inserted at the given index.
    ///
    /// ```candy
    /// ✨.listInsert (Foo, Bar) 0 Baz => (Baz, Foo, Bar)
    /// ✨.listInsert (Foo, Bar) 1 Baz => (Foo, Baz, Bar)
    /// ✨.listInsert (Foo, Bar) 2 Baz => (Foo, Bar, Baz)
    /// ```
    ListInsert,

    /// Takes a list. Returns the length of the list.
    ///
    /// ```candy
    /// ✨.listLength (Foo, Bar) => 2
    /// ```
    ListLength,

    /// Takes a list and an index (0 <= index < length of the list). Returns a
    /// two-item list with a new list without the item at the index, and the
    /// removed item.
    ///
    /// ```candy
    /// ✨.listRemoveAt (Foo, Bar, Baz) 1 => ((Foo, Baz), Bar)
    /// ```
    ListRemoveAt,

    /// Takes a list, an index (0 <= index < length of the list), and a new
    /// item. Returns a list where the item at the given index is replaced with
    /// the new item.
    ///
    /// ```candy
    /// ✨.listRemoveAt (Foo, Bar, Baz) 1 Blub => (Foo, Blub, Baz)
    /// ```
    ListReplace,

    /// Takes a text and prints it. Returns a `Nothing` tag.
    ///
    /// ```candy
    /// ✨.print "Hello!" => Nothing
    /// ```
    Print,

    /// Takes a struct and a key. The struct contains the key. Returns the value
    /// that's saved for that key.
    ///
    /// ```candy
    /// ✨.structGet [Foo: 2] Foo => 2
    /// ```
    StructGet,

    /// Takes a struct. Returns a list containing all of the struct keys in no
    /// guaranteed order.
    ///
    /// ```candy
    /// ✨.structGetKeys [Foo: 2, Bar: 1] => (Foo, Bar)
    /// ```
    StructGetKeys,

    /// Takes a struct and a key. Returns a boolean of whether the struct
    /// contains the key.
    ///
    /// ```candy
    /// ✨.structHasKey [Foo: 2, Bar: 1] Foo => True
    /// ✨.structHasKey [Foo: 2, Bar: 1] Bar => False
    /// ```
    StructHasKey,

    /// Takes a tag with a value. Returns the tag's value.
    ///
    /// ```candy
    /// ✨.tagGetValue (Foo 1) => 1
    /// ✨.tagGetValue (Bar Baz) => Baz
    /// ```
    TagGetValue,

    /// Takes a tag. Returns a boolean indicating whether the tag has a value.
    ///
    /// ```candy
    /// ✨.tagHasValue (Foo 1) => True
    /// ✨.tagHasValue Bar => False
    /// ```
    TagHasValue,

    /// Takes a tag. Returns the tag without a value.
    ///
    /// ```candy
    /// ✨.tagWithoutValue (Foo 1) => Foo
    /// ✨.tagWithoutValue Bar => Bar
    /// ```
    TagWithoutValue,

    /// Takes a text. Returns a list containing the individual characters.
    ///
    /// ```candy
    /// ✨.textCharacters "Hello" => ("H", "e", "l", "l", "o")
    /// ```
    TextCharacters,

    /// Takes two texts. Returns a concatenation of both inputs.
    ///
    /// ```candy
    /// ✨.textConcatenate "Hello" "world" => "Helloworld"
    /// ```
    TextConcatenate,

    /// Takes two texts. Returns whether the first text contains the second one.
    ///
    /// ```candy
    /// ✨.textContains "Hello" "H" => True
    /// ✨.textContains "Hello" "X" => False
    /// ```
    TextContains,

    /// Takes two texts. Returns whether the first text ends with the second
    /// one.
    ///
    /// ```candy
    /// ✨.textEndsWith "Hello, world" "world" => True
    /// ✨.textEndsWith "Hello, world" "you" => False
    /// ```
    TextEndsWith,

    /// Takes a list of integers representing bytes (so, 0 <= byte < 256 for
    /// each item of the list). If the bytes are a valid UTF-8 encoding, returns
    /// the corresponding text. Otherwise, returns `Error NotUtf8` and the
    /// original bytes.
    ///
    /// ```candy
    /// ✨.textFromUtf8 (104, 101, 108, 108, 111) => Ok "Hello"
    /// ✨.textFromUtf8 (104, 101, 245) => Error NotUtf8
    /// ```
    TextFromUtf8,

    /// Takes a text, and two integers, representing the inclusive start and
    /// exclusive end. Returns the substring of the text in that range.
    ///
    /// ```candy
    /// ✨.textGetRange "Hello" 1 3 => "el"
    /// ```
    TextGetRange,

    /// Takes a text. Returns whether it's empty.
    ///
    /// ```candy
    /// ✨.textIsEmpty "Hello" => False
    /// ✨.textIsEmpty "" => True
    /// ```
    TextIsEmpty,

    /// Takes a text. Returns the number of grapheme clusters in that text.
    ///
    /// ```candy
    /// ✨.textLength "Hello" => 5
    /// ✨.textLength "" => 0
    /// ```
    TextLength,

    /// Takes two texts. Returns whether the first text starts with the second
    /// one.
    ///
    /// ```candy
    /// ✨.textEndsWith "Hello, world" "Hello" => True
    /// ✨.textEndsWith "Hello, world" "Hi" => False
    /// ```
    TextStartsWith,

    /// Takes a text. Returns a text with whitespace removed at the end.
    ///
    /// ```candy
    /// ✨.textTrimEnd "  Hi  " => "  Hi"
    /// ```
    TextTrimEnd,

    /// Takes a text. Returns a text with whitespace removed at the start.
    ///
    /// ```candy
    /// ✨.textTrimStart "  Hi  " => "Hi  "
    /// ```
    TextTrimStart, // text -> text

    /// Takes a value. Returns a stringified version of the value.
    ///
    /// ```candy
    /// ✨.toDebugText 2 => "2"
    /// ```
    ToDebugText,

    /// Takes a value. Returns a tag denoting the type of the value. These are
    /// the possible types: `Function`, `Int`, `List`, `Struct`, `Text`, `Tag`
    ///
    /// ```candy
    /// ✨.typeOf {} => Function
    /// ✨.typeOf 2 => Int
    /// ✨.typeOf (1, 2) => List
    /// ✨.typeOf [Foo: 2] => Struct
    /// ✨.typeOf "Hi" => Text
    /// ✨.typeOf Text => Tag
    /// ```
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

use super::{
    object_heap::{
        function::HeapFunction, hir_id::HeapHirId, int::HeapInt, list::HeapList,
        struct_::HeapStruct, tag::HeapTag, text::HeapText, HeapData, HeapObject,
    },
    object_inline::{
        builtin::InlineBuiltin, handle::InlineHandle, int::InlineInt, tag::InlineTag, InlineData,
        InlineObject,
    },
    Heap,
};
use crate::{
    handle_id::HandleId,
    instruction_pointer::InstructionPointer,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::{builtin_functions::BuiltinFunction, hir::Id};
use derive_more::{Deref, From};
use num_bigint::BigInt;
use num_traits::Signed;
use rustc_hash::FxHashMap;
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{self, Debug, Formatter},
    hash::Hash,
    str,
};
use strum::{EnumDiscriminants, IntoStaticStr};

#[derive(Clone, Copy, EnumDiscriminants, Eq, Hash, IntoStaticStr, Ord, PartialEq, PartialOrd)]
#[strum_discriminants(derive(IntoStaticStr))]
pub enum Data {
    Int(Int),
    Tag(Tag),
    Text(Text),
    List(List),
    Struct(Struct),
    HirId(HirId),
    Function(Function),
    Builtin(Builtin),
    Handle(Handle),
}
impl Data {
    #[must_use]
    pub const fn function(&self) -> Option<&Function> {
        if let Self::Function(function) = self {
            Some(function)
        } else {
            None
        }
    }
}

impl From<InlineObject> for Data {
    fn from(object: InlineObject) -> Self {
        match object.into() {
            InlineData::Pointer(pointer) => pointer.get().into(),
            InlineData::Int(int) => Self::Int(Int::Inline(int)),
            InlineData::Builtin(builtin) => Self::Builtin(Builtin(builtin)),
            InlineData::Tag(symbol_id) => Self::Tag(Tag::Inline(symbol_id)),
            InlineData::Handle(handle) => Self::Handle(Handle(handle)),
        }
    }
}
impl From<HeapObject> for Data {
    fn from(object: HeapObject) -> Self {
        match object.into() {
            HeapData::Int(int) => Self::Int(Int::Heap(int)),
            HeapData::List(list) => Self::List(List(list)),
            HeapData::Struct(struct_) => Self::Struct(Struct(struct_)),
            HeapData::Tag(tag) => Self::Tag(Tag::Heap(tag)),
            HeapData::Text(text) => Self::Text(Text(text)),
            HeapData::Function(function) => Self::Function(Function(function)),
            HeapData::HirId(hir_id) => Self::HirId(HirId(hir_id)),
        }
    }
}

impl DebugDisplay for Data {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Self::Int(int) => DebugDisplay::fmt(int, f, is_debug),
            Self::Tag(tag) => DebugDisplay::fmt(tag, f, is_debug),
            Self::Text(text) => DebugDisplay::fmt(text, f, is_debug),
            Self::List(list) => DebugDisplay::fmt(list, f, is_debug),
            Self::Struct(struct_) => DebugDisplay::fmt(struct_, f, is_debug),
            Self::HirId(hir_id) => DebugDisplay::fmt(hir_id, f, is_debug),
            Self::Function(function) => DebugDisplay::fmt(function, f, is_debug),
            Self::Builtin(builtin) => DebugDisplay::fmt(builtin, f, is_debug),
            Self::Handle(send_port) => DebugDisplay::fmt(send_port, f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(Data);

// Int

// FIXME: Custom Ord, PartialOrd impl
#[derive(Clone, Copy, Eq, From, Hash, PartialEq)]
pub enum Int {
    Inline(InlineInt),
    Heap(HeapInt),
}

impl Int {
    #[must_use]
    pub fn create<T>(heap: &mut Heap, is_reference_counted: bool, value: T) -> Self
    where
        T: Copy + TryInto<i64> + Into<BigInt>,
    {
        value
            .try_into()
            .map_err(|_| ())
            .and_then(InlineInt::try_from)
            .map_or_else(
                |()| HeapInt::create(heap, is_reference_counted, value.into()).into(),
                Into::into,
            )
    }
    #[must_use]
    pub fn create_from_bigint(heap: &mut Heap, is_reference_counted: bool, value: BigInt) -> Self {
        i64::try_from(&value)
            .map_err(|_| ())
            .and_then(InlineInt::try_from)
            .map_or_else(
                |()| HeapInt::create(heap, is_reference_counted, value).into(),
                Into::into,
            )
    }

    #[must_use]
    pub fn get<'a>(self) -> Cow<'a, BigInt> {
        match self {
            Self::Inline(int) => Cow::Owned(int.get().into()),
            Self::Heap(int) => Cow::Borrowed(int.get()),
        }
    }
    #[must_use]
    pub fn try_get<T>(self) -> Option<T>
    where
        T: TryFrom<i64> + for<'a> TryFrom<&'a BigInt>,
    {
        match self {
            Self::Inline(int) => int.try_get(),
            Self::Heap(int) => int.get().try_into().ok(),
        }
    }

    operator_fn!(add);
    operator_fn!(subtract);
    operator_fn!(multiply);
    operator_fn!(int_divide_truncating);
    operator_fn!(remainder);
    #[must_use]
    pub fn modulo(self, heap: &mut Heap, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Inline(lhs), Self::Inline(rhs)) => lhs.modulo(heap, rhs),
            (Self::Heap(on_heap), Self::Inline(inline))
            | (Self::Inline(inline), Self::Heap(on_heap)) => {
                on_heap.modulo(heap, &inline.get().into())
            }
            (Self::Heap(lhs), Self::Heap(rhs)) => lhs.modulo(heap, rhs.get()),
        }
    }

    #[must_use]
    pub fn compare_to(self, heap: &Heap, rhs: Self) -> Tag {
        match (self, rhs) {
            (Self::Inline(lhs), rhs) => lhs.compare_to(heap, rhs),
            (Self::Heap(lhs), Self::Inline(rhs)) => lhs.compare_to(heap, &rhs.get().into()),
            (Self::Heap(lhs), Self::Heap(rhs)) => lhs.compare_to(heap, rhs.get()),
        }
    }

    #[must_use]
    pub fn shift_left(self, heap: &mut Heap, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Inline(lhs), Self::Inline(rhs)) => lhs.shift_left(heap, rhs),
            (Self::Inline(lhs), Self::Heap(rhs)) => Self::create_from_bigint(
                heap,
                true,
                // TODO: Support shifting by larger numbers
                BigInt::from(lhs.get()) << i128::try_from(rhs.get()).unwrap(),
            ),
            // TODO: Support shifting by larger numbers
            (Self::Heap(lhs), rhs) => lhs.shift_left(heap, rhs.try_get::<i128>().unwrap()),
        }
    }
    #[must_use]
    pub fn shift_right(self, heap: &mut Heap, rhs: Self) -> Self {
        match self {
            Self::Inline(lhs) => {
                let rhs = match rhs {
                    Self::Inline(rhs) => rhs,
                    Self::Heap(rhs) => {
                        debug_assert!(rhs.get().is_positive(), "Shift amount must be positive.");
                        #[allow(clippy::cast_possible_wrap)]
                        InlineInt::from_unchecked(InlineInt::VALUE_BITS as i64)
                    }
                };
                Self::Inline(lhs.shift_right(rhs))
            }
            // TODO: Support shifting by larger numbers
            Self::Heap(lhs) => lhs.shift_right(heap, rhs.try_get::<i128>().unwrap()),
        }
    }

    #[must_use]
    pub fn bit_length(self, heap: &mut Heap) -> Self {
        match self {
            Self::Inline(int) => int.bit_length().into(),
            Self::Heap(int) => int.bit_length(heap),
        }
    }

    bitwise_fn!(bitwise_and);
    bitwise_fn!(bitwise_or);
    bitwise_fn!(bitwise_xor);
}

macro_rules! bitwise_fn {
    ($name:ident) => {
        #[must_use]
        pub fn $name(self, heap: &mut Heap, rhs: Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), Int::Inline(rhs)) => lhs.$name(rhs).into(),
                (Int::Heap(on_heap), Int::Inline(inline))
                | (Int::Inline(inline), Int::Heap(on_heap)) => {
                    on_heap.$name(heap, &inline.get().into())
                }
                (Int::Heap(lhs), Int::Heap(rhs)) => lhs.$name(heap, rhs.get()),
            }
        }
    };
}
macro_rules! operator_fn {
    ($name:ident) => {
        #[must_use]
        pub fn $name(self, heap: &mut Heap, rhs: Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), _) => lhs.$name(heap, rhs),
                (Int::Heap(lhs), Int::Inline(rhs)) => lhs.$name(heap, rhs.get()),
                (Int::Heap(lhs), Int::Heap(rhs)) => lhs.$name(heap, rhs.get()),
            }
        }
    };
}
use {bitwise_fn, operator_fn};

impl DebugDisplay for Int {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Self::Inline(int) => DebugDisplay::fmt(int, f, is_debug),
            Self::Heap(int) => DebugDisplay::fmt(int, f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(Int);

impl From<Int> for InlineObject {
    fn from(value: Int) -> Self {
        match value {
            Int::Inline(int) => *int,
            Int::Heap(int) => (*int).into(),
        }
    }
}
impl_try_froms!(Int, "Expected an int.");
impl_try_from_heap_object!(Int, "Expected an int.");

impl Ord for Int {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Inline(this), Self::Inline(other)) => Ord::cmp(this, other),
            (Self::Inline(_), Self::Heap(other)) => {
                if other.get().is_positive() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (Self::Heap(this), Self::Heap(other)) => Ord::cmp(this, other),
            (Self::Heap(this), Self::Inline(_)) => {
                if this.get().is_positive() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
        }
    }
}
#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Int {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

// Tag

#[derive(Clone, Copy, Eq, From, Hash, PartialEq)]
pub enum Tag {
    Inline(InlineTag),
    Heap(HeapTag),
}

impl Tag {
    #[must_use]
    pub fn create(symbol: Text) -> Self {
        Self::Inline(InlineTag::new(symbol))
    }
    #[must_use]
    pub fn create_with_value(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol: Text,
        value: impl Into<InlineObject>,
    ) -> Self {
        HeapTag::create(heap, is_reference_counted, symbol, value).into()
    }
    #[must_use]
    pub fn create_with_value_option(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol: Text,
        value: impl Into<Option<InlineObject>>,
    ) -> Self {
        value.into().map_or_else(
            || Self::create(symbol),
            |value| Self::create_with_value(heap, is_reference_counted, symbol, value),
        )
    }
    #[must_use]
    pub fn create_nothing(heap: &Heap) -> Self {
        Self::create(heap.default_symbols().nothing)
    }
    #[must_use]
    pub fn create_bool(heap: &Heap, value: bool) -> Self {
        let symbol = if value {
            heap.default_symbols().true_
        } else {
            heap.default_symbols().false_
        };
        Self::create(symbol)
    }
    #[must_use]
    pub fn create_ordering(heap: &Heap, value: Ordering) -> Self {
        let value = match value {
            Ordering::Less => heap.default_symbols().less,
            Ordering::Equal => heap.default_symbols().equal,
            Ordering::Greater => heap.default_symbols().greater,
        };
        Self::create(value)
    }
    #[must_use]
    pub fn create_result(
        heap: &mut Heap,
        is_reference_counted: bool,
        value: Result<InlineObject, InlineObject>,
    ) -> Self {
        let (symbol, value) = match value {
            Ok(it) => (heap.default_symbols().ok, it),
            Err(it) => (heap.default_symbols().error, it),
        };
        Self::create_with_value(heap, is_reference_counted, symbol, value)
    }

    #[must_use]
    pub fn symbol(&self) -> Text {
        match self {
            Self::Inline(tag) => tag.get(),
            Self::Heap(tag) => tag.symbol(),
        }
    }
    pub fn try_into_bool(self, heap: &Heap) -> Result<bool, &'static str> {
        match self {
            Self::Inline(tag) => {
                let symbol = tag.get();
                if symbol == heap.default_symbols().true_ {
                    Ok(true)
                } else if symbol == heap.default_symbols().false_ {
                    Ok(false)
                } else {
                    Err("Expected `True` or `False`.")
                }
            }
            Self::Heap(_) => Err("Expected a tag without a value, found {value:?}."),
        }
    }

    #[must_use]
    pub const fn has_value(&self) -> bool {
        match self {
            Self::Inline(_) => false,
            Self::Heap(_) => true,
        }
    }
    #[must_use]
    pub fn value(&self) -> Option<InlineObject> {
        match self {
            Self::Inline(_) => None,
            Self::Heap(tag) => Some(tag.value()),
        }
    }

    #[must_use]
    pub fn without_value(self) -> Self {
        Self::create(self.symbol())
    }
}

impl DebugDisplay for Tag {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Self::Inline(tag) => DebugDisplay::fmt(tag, f, is_debug),
            Self::Heap(tag) => DebugDisplay::fmt(tag, f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(Tag);

impl From<Tag> for InlineObject {
    fn from(value: Tag) -> Self {
        match value {
            Tag::Inline(value) => *value,
            Tag::Heap(value) => (*value).into(),
        }
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol()
            .cmp(&other.symbol())
            .then_with(|| self.value().cmp(&other.value()))
    }
}
impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl_try_froms!(Tag, "Expected a tag.");
impl_try_from_heap_object!(Tag, "Expected a tag.");

// Text

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text(HeapText);

impl Text {
    #[must_use]
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: &str) -> Self {
        HeapText::create(heap, is_reference_counted, value).into()
    }
}

impls_via_0!(Text);
impl_try_froms!(Text, "Expected a text.");
impl_try_from_heap_object!(Text, "Expected a text.");

// List

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct List(HeapList);

impl List {
    #[must_use]
    pub fn create(heap: &mut Heap, is_reference_counted: bool, items: &[InlineObject]) -> Self {
        HeapList::create(heap, is_reference_counted, items).into()
    }
}

impls_via_0!(List);
impl_try_froms!(List, "Expected a list.");
impl_try_from_heap_object!(List, "Expected a list.");

// Struct

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Struct(HeapStruct);

impl Struct {
    #[must_use]
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        fields: &FxHashMap<InlineObject, InlineObject>,
    ) -> Self {
        HeapStruct::create(heap, is_reference_counted, fields).into()
    }
    #[must_use]
    pub fn create_with_symbol_keys(
        heap: &mut Heap,
        is_reference_counted: bool,
        fields: impl IntoIterator<Item = (Text, InlineObject)>,
    ) -> Self {
        let fields = fields
            .into_iter()
            .map(|(key, value)| ((Tag::create(key)).into(), value))
            .collect();
        Self::create(heap, is_reference_counted, &fields)
    }
}

impls_via_0!(Struct);
impl_try_froms!(Struct, "Expected a struct.");
impl_try_from_heap_object!(Struct, "Expected a struct.");

// Function

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Function(HeapFunction);

impl Function {
    #[must_use]
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        captured: &[InlineObject],
        argument_count: usize,
        body: InstructionPointer,
    ) -> Self {
        HeapFunction::create(heap, is_reference_counted, captured, argument_count, body).into()
    }
}

impls_via_0!(Function);
impl_try_froms!(Function, "Expected a function.");
impl_try_from_heap_object!(Function, "Expected a function.");

// HIR ID

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]

pub struct HirId(HeapHirId);

impl HirId {
    #[must_use]
    pub fn create(heap: &mut Heap, is_reference_counted: bool, id: Id) -> Self {
        HeapHirId::create(heap, is_reference_counted, id).into()
    }
}

impls_via_0!(HirId);
impl_try_froms!(HirId, "Expected a HIR ID.");
impl_try_from_heap_object!(HirId, "Expected a HIR ID.");

// Builtin

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Builtin(InlineBuiltin);

impl Builtin {
    #[must_use]
    pub fn create(builtin: BuiltinFunction) -> Self {
        InlineBuiltin::from(builtin).into()
    }
}

impls_via_0!(Builtin);

// Handle

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Handle(InlineHandle);

impl Handle {
    #[must_use]
    pub fn new(heap: &mut Heap, argument_count: usize) -> Self {
        let id = heap.handle_id_generator.generate();
        Self::create(heap, id, argument_count)
    }
    #[must_use]
    pub fn create(heap: &mut Heap, handle_id: HandleId, argument_count: usize) -> Self {
        InlineHandle::create(heap, handle_id, argument_count).into()
    }
}

impls_via_0!(Handle);
impl_try_froms!(Handle, "Expected a handle.");

// Utils

macro_rules! impls_via_0 {
    ($type:ty) => {
        impl DebugDisplay for $type {
            fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
                DebugDisplay::fmt(&self.0, f, is_debug)
            }
        }
        impl_debug_display_via_debugdisplay!($type);

        impl From<$type> for InlineObject {
            fn from(value: $type) -> Self {
                (**value).into()
            }
        }
    };
}

macro_rules! impl_try_froms {
    ($type:tt, $error_message:expr$(,)?) => {
        impl TryFrom<InlineObject> for $type {
            type Error = &'static str;

            fn try_from(value: InlineObject) -> Result<Self, Self::Error> {
                Data::from(value).try_into()
            }
        }
        impl TryFrom<Data> for $type {
            type Error = &'static str;

            fn try_from(value: Data) -> Result<Self, Self::Error> {
                match value {
                    Data::$type(it) => Ok(it),
                    _ => Err($error_message),
                }
            }
        }
        impl<'a> TryFrom<&'a Data> for &'a $type {
            type Error = &'static str;

            fn try_from(value: &'a Data) -> Result<Self, Self::Error> {
                match &value {
                    Data::$type(it) => Ok(it),
                    _ => Err($error_message),
                }
            }
        }
    };
}
macro_rules! impl_try_from_heap_object {
    ($type:tt, $error_message:expr$(,)?) => {
        impl TryFrom<HeapObject> for $type {
            type Error = &'static str;

            fn try_from(value: HeapObject) -> Result<Self, Self::Error> {
                Data::from(value).try_into()
            }
        }
    };
}
use {impl_try_from_heap_object, impl_try_froms, impls_via_0};

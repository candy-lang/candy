use super::{
    object_heap::{
        function::HeapFunction, hir_id::HeapHirId, int::HeapInt, list::HeapList,
        struct_::HeapStruct, tag::HeapTag, text::HeapText, HeapData, HeapObject,
    },
    object_inline::{
        builtin::InlineBuiltin, handle::InlineHandle, int::InlineInt, tag::InlineTag, InlineData,
        InlineObject,
    },
    symbol_table::{impl_ord_with_symbol_table_via_ord, DisplayWithSymbolTable},
    Heap, OrdWithSymbolTable, SymbolId, SymbolTable,
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
    intrinsics,
    ops::{Shl, Shr},
    str,
};
use strum::{EnumDiscriminants, IntoStaticStr};

#[derive(Clone, Copy, EnumDiscriminants, Eq, Hash, IntoStaticStr, PartialEq)]
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
    pub fn function(&self) -> Option<&Function> {
        if let Data::Function(function) = self {
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
            InlineData::Int(int) => Data::Int(Int::Inline(int)),
            InlineData::Builtin(builtin) => Data::Builtin(Builtin(builtin)),
            InlineData::Tag(symbol_id) => Data::Tag(Tag::Inline(symbol_id)),
            InlineData::Handle(handle) => Data::Handle(Handle(handle)),
        }
    }
}
impl From<HeapObject> for Data {
    fn from(object: HeapObject) -> Self {
        match object.into() {
            HeapData::Int(int) => Data::Int(Int::Heap(int)),
            HeapData::List(list) => Data::List(List(list)),
            HeapData::Struct(struct_) => Data::Struct(Struct(struct_)),
            HeapData::Tag(tag) => Data::Tag(Tag::Heap(tag)),
            HeapData::Text(text) => Data::Text(Text(text)),
            HeapData::Function(function) => Data::Function(Function(function)),
            HeapData::HirId(hir_id) => Data::HirId(HirId(hir_id)),
        }
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Data::Int(int) => Debug::fmt(int, f),
            Data::Tag(tag) => Debug::fmt(tag, f),
            Data::Text(text) => Debug::fmt(text, f),
            Data::List(list) => Debug::fmt(list, f),
            Data::Struct(struct_) => Debug::fmt(struct_, f),
            Data::HirId(hir_id) => Debug::fmt(hir_id, f),
            Data::Function(function) => Debug::fmt(function, f),
            Data::Builtin(builtin) => Debug::fmt(builtin, f),
            Data::Handle(handle) => Debug::fmt(handle, f),
        }
    }
}

impl DisplayWithSymbolTable for Data {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        match self {
            Data::Int(int) => DisplayWithSymbolTable::fmt(int, f, symbol_table),
            Data::Tag(tag) => DisplayWithSymbolTable::fmt(tag, f, symbol_table),
            Data::Text(text) => DisplayWithSymbolTable::fmt(text, f, symbol_table),
            Data::List(list) => DisplayWithSymbolTable::fmt(list, f, symbol_table),
            Data::Struct(struct_) => DisplayWithSymbolTable::fmt(struct_, f, symbol_table),
            Data::HirId(hir_id) => DisplayWithSymbolTable::fmt(hir_id, f, symbol_table),
            Data::Function(function) => DisplayWithSymbolTable::fmt(function, f, symbol_table),
            Data::Builtin(builtin) => DisplayWithSymbolTable::fmt(builtin, f, symbol_table),
            Data::Handle(handle) => DisplayWithSymbolTable::fmt(handle, f, symbol_table),
        }
    }
}

impl OrdWithSymbolTable for Data {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        match (self, other) {
            (Data::Int(this), Data::Int(other)) => Ord::cmp(this, other),
            (Data::Tag(this), Data::Tag(other)) => {
                OrdWithSymbolTable::cmp(this, symbol_table, other)
            }
            (Data::Text(this), Data::Text(other)) => Ord::cmp(this, other),
            (Data::List(this), Data::List(other)) => {
                OrdWithSymbolTable::cmp(this, symbol_table, other)
            }
            (Data::Struct(this), Data::Struct(other)) => {
                OrdWithSymbolTable::cmp(this, symbol_table, other)
            }
            (Data::HirId(this), Data::HirId(other)) => Ord::cmp(this, other),
            (Data::Function(this), Data::Function(other)) => Ord::cmp(this, other),
            (Data::Builtin(this), Data::Builtin(other)) => Ord::cmp(this, other),
            (Data::Handle(this), Data::Handle(other)) => Ord::cmp(this, other),
            _ => intrinsics::discriminant_value(self).cmp(&intrinsics::discriminant_value(other)),
        }
    }
}

// Int

// FIXME: Custom Ord, PartialOrd impl
#[derive(Clone, Copy, Eq, From, Hash, PartialEq)]
pub enum Int {
    Inline(InlineInt),
    Heap(HeapInt),
}

impl Int {
    pub fn create<T>(heap: &mut Heap, is_reference_counted: bool, value: T) -> Self
    where
        T: Copy + TryInto<i64> + Into<BigInt>,
    {
        value
            .try_into()
            .map_err(|_| ())
            .and_then(InlineInt::try_from)
            .map(|it| it.into())
            .unwrap_or_else(|_| HeapInt::create(heap, is_reference_counted, value.into()).into())
    }
    pub fn create_from_bigint(heap: &mut Heap, is_reference_counted: bool, value: BigInt) -> Self {
        i64::try_from(&value)
            .map_err(|_| ())
            .and_then(InlineInt::try_from)
            .map(|it| it.into())
            .unwrap_or_else(|_| HeapInt::create(heap, is_reference_counted, value).into())
    }

    pub fn get<'a>(self) -> Cow<'a, BigInt> {
        match self {
            Int::Inline(int) => Cow::Owned(int.get().into()),
            Int::Heap(int) => Cow::Borrowed(int.get()),
        }
    }
    pub fn try_get<T>(self) -> Option<T>
    where
        T: TryFrom<i64> + for<'a> TryFrom<&'a BigInt>,
    {
        match self {
            Int::Inline(int) => int.try_get(),
            Int::Heap(int) => int.get().try_into().ok(),
        }
    }

    operator_fn!(add);
    operator_fn!(subtract);
    operator_fn!(multiply);
    operator_fn!(int_divide_truncating);
    operator_fn!(remainder);
    pub fn modulo(self, heap: &mut Heap, rhs: Int) -> Self {
        match (self, rhs) {
            (Int::Inline(lhs), Int::Inline(rhs)) => lhs.modulo(heap, rhs),
            (Int::Heap(on_heap), Int::Inline(inline))
            | (Int::Inline(inline), Int::Heap(on_heap)) => {
                on_heap.modulo(heap, &inline.get().into())
            }
            (Int::Heap(lhs), Int::Heap(rhs)) => lhs.modulo(heap, rhs.get()),
        }
    }

    pub fn compare_to(self, rhs: Int) -> Tag {
        match (self, rhs) {
            (Int::Inline(lhs), rhs) => lhs.compare_to(rhs),
            (Int::Heap(lhs), Int::Inline(rhs)) => lhs.compare_to(&rhs.get().into()),
            (Int::Heap(lhs), Int::Heap(rhs)) => lhs.compare_to(rhs.get()),
        }
    }

    shift_fn!(shift_left, shl);
    shift_fn!(shift_right, shr);

    pub fn bit_length(self, heap: &mut Heap) -> Self {
        match self {
            Int::Inline(int) => int.bit_length().into(),
            Int::Heap(int) => int.bit_length(heap),
        }
    }

    bitwise_fn!(bitwise_and);
    bitwise_fn!(bitwise_or);
    bitwise_fn!(bitwise_xor);
}

macro_rules! bitwise_fn {
    ($name:ident) => {
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
        pub fn $name(self, heap: &mut Heap, rhs: Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), _) => lhs.$name(heap, rhs),
                (Int::Heap(lhs), Int::Inline(rhs)) => lhs.$name(heap, rhs.get()),
                (Int::Heap(lhs), Int::Heap(rhs)) => lhs.$name(heap, rhs.get()),
            }
        }
    };
}
macro_rules! shift_fn {
    ($name:ident, $function:ident) => {
        pub fn $name(self, heap: &mut Heap, rhs: Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), Int::Inline(rhs)) => lhs.$name(heap, rhs),
                // TODO: Support shifting by larger numbers
                (Int::Inline(lhs), rhs) => Int::create_from_bigint(
                    heap,
                    true,
                    BigInt::from(lhs.get()).$function(rhs.try_get::<i128>().unwrap()),
                ),
                (Int::Heap(lhs), rhs) => lhs.$name(heap, rhs.try_get::<i128>().unwrap()),
            }
        }
    };
}
use {bitwise_fn, operator_fn, shift_fn};

impl DebugDisplay for Int {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Int::Inline(int) => DebugDisplay::fmt(int, f, is_debug),
            Int::Heap(int) => DebugDisplay::fmt(int, f, is_debug),
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
            (Int::Inline(this), Int::Inline(other)) => Ord::cmp(this, other),
            (Int::Inline(_), Int::Heap(other)) => {
                if other.get().is_positive() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (Int::Heap(this), Int::Heap(other)) => Ord::cmp(this, other),
            (Int::Heap(this), Int::Inline(_)) => {
                if this.get().is_positive() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
        }
    }
}
#[allow(clippy::incorrect_partial_ord_impl_on_ord_type)]
impl PartialOrd for Int {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl_ord_with_symbol_table_via_ord!(Int);

// Tag

#[derive(Clone, Copy, Eq, From, Hash, PartialEq)]
pub enum Tag {
    Inline(InlineTag),
    Heap(HeapTag),
}

impl Tag {
    pub fn create(symbol_id: SymbolId) -> Self {
        Tag::Inline(symbol_id.into())
    }
    pub fn create_with_value(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol_id: SymbolId,
        value: impl Into<InlineObject>,
    ) -> Self {
        HeapTag::create(heap, is_reference_counted, symbol_id, value).into()
    }
    pub fn create_with_value_option(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol_id: SymbolId,
        value: impl Into<Option<InlineObject>>,
    ) -> Self {
        match value.into() {
            Some(value) => Self::create_with_value(heap, is_reference_counted, symbol_id, value),
            None => Self::create(symbol_id),
        }
    }
    pub fn create_nothing() -> Self {
        Self::create(SymbolId::NOTHING)
    }
    pub fn create_bool(value: bool) -> Self {
        let symbol_id = if value {
            SymbolId::TRUE
        } else {
            SymbolId::FALSE
        };
        Self::create(symbol_id)
    }
    pub fn create_ordering(value: Ordering) -> Self {
        let value = match value {
            Ordering::Less => SymbolId::LESS,
            Ordering::Equal => SymbolId::EQUAL,
            Ordering::Greater => SymbolId::GREATER,
        };
        Self::create(value)
    }
    pub fn create_result(
        heap: &mut Heap,
        is_reference_counted: bool,
        value: Result<InlineObject, InlineObject>,
    ) -> Self {
        let (symbol, value) = match value {
            Ok(it) => (SymbolId::OK, it),
            Err(it) => (SymbolId::ERROR, it),
        };
        Self::create_with_value(heap, is_reference_counted, symbol, value)
    }

    pub fn symbol_id(&self) -> SymbolId {
        match self {
            Tag::Inline(tag) => tag.get(),
            Tag::Heap(tag) => tag.symbol_id(),
        }
    }
    pub fn has_value(&self) -> bool {
        match self {
            Tag::Inline(_) => false,
            Tag::Heap(_) => true,
        }
    }
    pub fn value(&self) -> Option<InlineObject> {
        match self {
            Tag::Inline(_) => None,
            Tag::Heap(tag) => Some(tag.value()),
        }
    }

    pub fn without_value(self) -> Tag {
        Tag::create(self.symbol_id())
    }
}

impl Debug for Tag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Tag::Inline(tag) => Debug::fmt(tag, f),
            Tag::Heap(tag) => Debug::fmt(tag, f),
        }
    }
}
impl DisplayWithSymbolTable for Tag {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        match self {
            Tag::Inline(tag) => DisplayWithSymbolTable::fmt(tag, f, symbol_table),
            Tag::Heap(tag) => DisplayWithSymbolTable::fmt(tag, f, symbol_table),
        }
    }
}

impl From<Tag> for InlineObject {
    fn from(value: Tag) -> Self {
        match value {
            Tag::Inline(value) => *value,
            Tag::Heap(value) => (*value).into(),
        }
    }
}

impl OrdWithSymbolTable for Tag {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        symbol_table
            .get(self.symbol_id())
            .cmp(symbol_table.get(other.symbol_id()))
            .then_with(|| self.value().cmp(symbol_table, &other.value()))
    }
}

impl_try_froms!(Tag, "Expected a tag.");
impl_try_from_heap_object!(Tag, "Expected a tag.");

impl TryFrom<InlineObject> for bool {
    type Error = &'static str;

    fn try_from(value: InlineObject) -> Result<Self, Self::Error> {
        (Data::from(value)).try_into()
    }
}
impl TryFrom<Data> for bool {
    type Error = &'static str;

    fn try_from(value: Data) -> Result<Self, Self::Error> {
        match value {
            Data::Tag(Tag::Inline(tag)) => match tag.get() {
                SymbolId::TRUE => Ok(true),
                SymbolId::FALSE => Ok(false),
                _ => Err("Expected `True` or `False`."),
            },
            _ => Err("Expected a tag without a value, found {value:?}."),
        }
    }
}

// Text

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text(HeapText);

impl Text {
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: &str) -> Self {
        HeapText::create(heap, is_reference_counted, value).into()
    }
    pub fn create_from_utf8(heap: &mut Heap, is_reference_counted: bool, bytes: &[u8]) -> Tag {
        let result = str::from_utf8(bytes)
            .map(|it| Text::create(heap, is_reference_counted, it).into())
            .map_err(|_| Text::create(heap, is_reference_counted, "Invalid UTF-8.").into());
        Tag::create_result(heap, is_reference_counted, result)
    }
}

impls_via_0!(Text);
impl_try_froms!(Text, "Expected a text.");
impl_try_from_heap_object!(Text, "Expected a text.");
impl_ord_with_symbol_table_via_ord!(Text);

// List

#[derive(Clone, Copy, Deref, Eq, From, Hash, PartialEq)]
pub struct List(HeapList);

impl List {
    pub fn create(heap: &mut Heap, is_reference_counted: bool, items: &[InlineObject]) -> Self {
        HeapList::create(heap, is_reference_counted, items).into()
    }
}

impls_via_0_with_symbol_table!(List);
impl_try_froms!(List, "Expected a list.");
impl_try_from_heap_object!(List, "Expected a list.");

// Struct

#[derive(Clone, Copy, Deref, Eq, From, Hash, PartialEq)]
pub struct Struct(HeapStruct);

impl Struct {
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        fields: &FxHashMap<InlineObject, InlineObject>,
    ) -> Self {
        HeapStruct::create(heap, is_reference_counted, fields).into()
    }
    pub fn create_with_symbol_keys(
        heap: &mut Heap,
        is_reference_counted: bool,
        fields: impl IntoIterator<Item = (SymbolId, InlineObject)>,
    ) -> Self {
        let fields = fields
            .into_iter()
            .map(|(key, value)| ((Tag::create(key)).into(), value))
            .collect();
        Self::create(heap, is_reference_counted, &fields)
    }
}

impls_via_0_with_symbol_table!(Struct);
impl_try_froms!(Struct, "Expected a struct.");
impl_try_from_heap_object!(Struct, "Expected a struct.");

// Function

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Function(HeapFunction);

impl Function {
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
impl_ord_with_symbol_table_via_ord!(Function);

// HIR ID

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]

pub struct HirId(HeapHirId);

impl HirId {
    pub fn create(heap: &mut Heap, is_reference_counted: bool, id: Id) -> HirId {
        HeapHirId::create(heap, is_reference_counted, id).into()
    }
}

impls_via_0!(HirId);
impl_try_froms!(HirId, "Expected a HIR ID.");
impl_try_from_heap_object!(HirId, "Expected a HIR ID.");
impl_ord_with_symbol_table_via_ord!(HirId);

// Builtin

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Builtin(InlineBuiltin);

impl Builtin {
    pub fn create(builtin: BuiltinFunction) -> Self {
        InlineBuiltin::from(builtin).into()
    }
}

impls_via_0!(Builtin);
impl_ord_with_symbol_table_via_ord!(Builtin);

// Handle

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Handle(InlineHandle);

impl Handle {
    pub fn new(heap: &mut Heap, argument_count: usize) -> Self {
        let id = heap.handle_id_generator.generate();
        Self::create(heap, id, argument_count)
    }
    pub fn create(heap: &mut Heap, handle_id: HandleId, argument_count: usize) -> Self {
        InlineHandle::create(heap, handle_id, argument_count).into()
    }
}

impls_via_0!(Handle);
impl_ord_with_symbol_table_via_ord!(Handle);
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
macro_rules! impls_via_0_with_symbol_table {
    ($type:ty) => {
        impl Debug for $type {
            fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                Debug::fmt(&self.0, f)
            }
        }
        impl DisplayWithSymbolTable for $type {
            fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
                DisplayWithSymbolTable::fmt(&self.0, f, symbol_table)
            }
        }

        impl From<$type> for InlineObject {
            fn from(value: $type) -> Self {
                (**value).into()
            }
        }

        impl OrdWithSymbolTable for $type {
            fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
                OrdWithSymbolTable::cmp(&self.0, symbol_table, &other.0)
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
use {impl_try_from_heap_object, impl_try_froms, impls_via_0, impls_via_0_with_symbol_table};

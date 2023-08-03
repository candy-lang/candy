use super::{
    object_heap::{
        function::HeapFunction, hir_id::HeapHirId, int::HeapInt, list::HeapList,
        struct_::HeapStruct, tag::HeapTag, text::HeapText, HeapData, HeapObject,
    },
    object_inline::{
        builtin::InlineBuiltin,
        int::InlineInt,
        port::{InlineReceivePort, InlineSendPort},
        InlineData, InlineObject,
    },
    symbol_table::{impl_ops_with_symbol_table_via_ops, DisplayWithSymbolTable},
    Heap, OrdWithSymbolTable, SymbolId, SymbolTable,
};
use crate::{
    channel::ChannelId,
    fiber::InstructionPointer,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::{builtin_functions::BuiltinFunction, hir::Id};
use derive_more::{Deref, From};
use num_bigint::BigInt;
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
    SendPort(SendPort),
    ReceivePort(ReceivePort),
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
            InlineData::SendPort(send_port) => Data::SendPort(SendPort(send_port)),
            InlineData::ReceivePort(receive_port) => Data::ReceivePort(ReceivePort(receive_port)),
            InlineData::Builtin(builtin) => Data::Builtin(Builtin(builtin)),
        }
    }
}
impl From<HeapObject> for Data {
    fn from(object: HeapObject) -> Self {
        match object.into() {
            HeapData::Int(int) => Data::Int(Int::Heap(int)),
            HeapData::List(list) => Data::List(List(list)),
            HeapData::Struct(struct_) => Data::Struct(Struct(struct_)),
            HeapData::Tag(tag) => Data::Tag(Tag(tag)),
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
            Data::SendPort(send_port) => Debug::fmt(send_port, f),
            Data::ReceivePort(receive_port) => Debug::fmt(receive_port, f),
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
            Data::SendPort(send_port) => DisplayWithSymbolTable::fmt(send_port, f, symbol_table),
            Data::ReceivePort(receive_port) => {
                DisplayWithSymbolTable::fmt(receive_port, f, symbol_table)
            }
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
            (Data::SendPort(this), Data::SendPort(other)) => Ord::cmp(this, other),
            (Data::ReceivePort(this), Data::ReceivePort(other)) => Ord::cmp(this, other),
            _ => intrinsics::discriminant_value(self).cmp(&intrinsics::discriminant_value(other)),
        }
    }
}

// Int

#[derive(Clone, Copy, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
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

    pub fn get(&self) -> Cow<BigInt> {
        match self {
            Int::Inline(int) => Cow::Owned(int.get().into()),
            Int::Heap(int) => Cow::Borrowed(int.get()),
        }
    }
    pub fn try_get<T>(&self) -> Option<T>
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
    pub fn modulo(&self, heap: &mut Heap, rhs: &Int) -> Self {
        match (self, rhs) {
            (Int::Inline(lhs), Int::Inline(rhs)) => lhs.modulo(heap, *rhs),
            (Int::Heap(on_heap), Int::Inline(inline))
            | (Int::Inline(inline), Int::Heap(on_heap)) => {
                on_heap.modulo(heap, &inline.get().into())
            }
            (Int::Heap(lhs), Int::Heap(rhs)) => lhs.modulo(heap, rhs.get()),
        }
    }

    pub fn compare_to(&self, heap: &mut Heap, rhs: &Int) -> Tag {
        match (self, rhs) {
            (Int::Inline(lhs), rhs) => lhs.compare_to(heap, *rhs),
            (Int::Heap(lhs), Int::Inline(rhs)) => lhs.compare_to(heap, &rhs.get().into()),
            (Int::Heap(lhs), Int::Heap(rhs)) => lhs.compare_to(heap, rhs.get()),
        }
    }

    shift_fn!(shift_left, shl);
    shift_fn!(shift_right, shr);

    pub fn bit_length(&self, heap: &mut Heap) -> Self {
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
        pub fn $name(&self, heap: &mut Heap, rhs: &Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), Int::Inline(rhs)) => lhs.$name(*rhs).into(),
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
        pub fn $name(&self, heap: &mut Heap, rhs: &Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), _) => lhs.$name(heap, *rhs),
                (Int::Heap(lhs), Int::Inline(rhs)) => lhs.$name(heap, rhs.get()),
                (Int::Heap(lhs), Int::Heap(rhs)) => lhs.$name(heap, rhs.get()),
            }
        }
    };
}
macro_rules! shift_fn {
    ($name:ident, $function:ident) => {
        pub fn $name(&self, heap: &mut Heap, rhs: &Int) -> Self {
            match (self, rhs) {
                (Int::Inline(lhs), Int::Inline(rhs)) => lhs.$name(heap, *rhs),
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
    fn from(int: Int) -> Self {
        match int {
            Int::Inline(int) => *int,
            Int::Heap(int) => (*int).into(),
        }
    }
}
impl_try_froms!(Int, "Expected an int.");
impl_try_from_heap_object!(Int, "Expected an int.");
impl_ops_with_symbol_table_via_ops!(Int);

// Tag

#[derive(Clone, Copy, Deref, Eq, From, Hash, PartialEq)]
pub struct Tag(HeapTag);

impl Tag {
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol_id: SymbolId,
        value: impl Into<Option<InlineObject>>,
    ) -> Self {
        HeapTag::create(heap, is_reference_counted, symbol_id, value).into()
    }
    pub fn create_nothing(heap: &mut Heap, is_reference_counted: bool) -> Self {
        Self::create(heap, is_reference_counted, SymbolId::NOTHING, None)
    }
    pub fn create_bool(heap: &mut Heap, is_reference_counted: bool, value: bool) -> Self {
        Self::create(
            heap,
            is_reference_counted,
            if value {
                SymbolId::TRUE
            } else {
                SymbolId::FALSE
            },
            None,
        )
    }
    pub fn create_ordering(heap: &mut Heap, is_reference_counted: bool, value: Ordering) -> Self {
        let value = match value {
            Ordering::Less => SymbolId::LESS,
            Ordering::Equal => SymbolId::EQUAL,
            Ordering::Greater => SymbolId::GREATER,
        };
        Self::create(heap, is_reference_counted, value, None)
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
        Self::create(heap, is_reference_counted, symbol, value)
    }
}

impls_via_0_with_symbol_table!(Tag);
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
        let tag: Tag = value.try_into()?;
        if tag.value().is_some() {
            return Err("Expected a tag without a value.");
        }

        match tag.symbol_id() {
            SymbolId::TRUE => Ok(true),
            SymbolId::FALSE => Ok(false),
            _ => Err("Expected `True` or `False`."),
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
impl_ops_with_symbol_table_via_ops!(Text);

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
            .map(|(key, value)| {
                (
                    (Tag::create(heap, is_reference_counted, key, None)).into(),
                    value,
                )
            })
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
impl_ops_with_symbol_table_via_ops!(Function);

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
impl_ops_with_symbol_table_via_ops!(HirId);

// Builtin

#[derive(Clone, Copy, Deref, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
pub struct Builtin(InlineBuiltin);

impl Builtin {
    pub fn create(builtin: BuiltinFunction) -> Self {
        InlineBuiltin::from(builtin).into()
    }
}

impls_via_0!(Builtin);
impl_ops_with_symbol_table_via_ops!(Builtin);

// Send Port

#[derive(Clone, Copy, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SendPort(InlineSendPort);

impl SendPort {
    pub fn create(heap: &mut Heap, channel_id: ChannelId) -> InlineObject {
        InlineSendPort::create(heap, channel_id)
    }
}

impls_via_0!(SendPort);
impl_ops_with_symbol_table_via_ops!(SendPort);
impl_try_froms!(SendPort, "Expected a send port.");

// Receive Port

#[derive(Clone, Copy, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReceivePort(InlineReceivePort);

impl ReceivePort {
    pub fn create(heap: &mut Heap, channel_id: ChannelId) -> InlineObject {
        InlineReceivePort::create(heap, channel_id)
    }
}

impls_via_0!(ReceivePort);
impl_ops_with_symbol_table_via_ops!(ReceivePort);
impl_try_froms!(ReceivePort, "Expected a receive port.");

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

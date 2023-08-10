use self::{
    builtin::InlineBuiltin,
    int::InlineInt,
    pointer::InlinePointer,
    port::{InlineReceivePort, InlineSendPort},
    tag::InlineTag,
};
use super::{
    object_heap::HeapObject, Data, DisplayWithSymbolTable, Heap, OrdWithSymbolTable, SymbolTable,
};
use crate::channel::ChannelId;
use candy_frontend::format::{format_value, FormatValue, MaxLength, Precedence};
use enum_dispatch::enum_dispatch;
use extension_trait::extension_trait;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ops::Deref,
};

pub(super) mod builtin;
pub(super) mod int;
pub(super) mod pointer;
pub(super) mod port;
pub(super) mod tag;

#[extension_trait]
pub impl InlineObjectSliceCloneToHeap for [InlineObject] {
    fn clone_to_heap(&self, heap: &mut Heap) -> Vec<InlineObject> {
        self.clone_to_heap_with_mapping(heap, &mut FxHashMap::default())
    }
    fn clone_to_heap_with_mapping(
        &self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Vec<InlineObject> {
        self.iter()
            .map(|&item| item.clone_to_heap_with_mapping(heap, address_map))
            .collect()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct InlineObject(NonZeroU64);

impl InlineObject {
    pub const KIND_WIDTH: usize = 3;
    pub const KIND_MASK: u64 = 0b111;
    pub const KIND_POINTER: u64 = 0b000;
    pub const KIND_INT: u64 = 0b001;
    pub const KIND_BUILTIN: u64 = 0b010;
    pub const KIND_TAG: u64 = 0b011;
    pub const KIND_SEND_PORT: u64 = 0b100;
    pub const KIND_RECEIVE_PORT: u64 = 0b101;

    pub fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    pub fn raw_word(self) -> NonZeroU64 {
        self.0
    }

    // Reference Counting
    pub fn dup(self, heap: &mut Heap) {
        self.dup_by(heap, 1);
    }
    pub fn dup_by(self, heap: &mut Heap, amount: usize) {
        if let Some(channel) = InlineData::from(self).channel_id() {
            heap.dup_channel_by(channel, amount);
        };

        if let Ok(it) = HeapObject::try_from(self) {
            it.dup_by(heap, amount)
        }
    }
    pub fn drop(self, heap: &mut Heap) {
        if let Some(channel) = InlineData::from(self).channel_id() {
            heap.drop_channel(channel);
        };

        if let Ok(it) = HeapObject::try_from(self) {
            it.drop(heap)
        }
    }

    // Cloning
    pub fn clone_to_heap(self, heap: &mut Heap) -> Self {
        self.clone_to_heap_with_mapping(heap, &mut FxHashMap::default())
    }
    pub fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        *InlineData::from(self).clone_to_heap_with_mapping(heap, address_map)
    }
}

impl Debug for InlineObject {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&InlineData::from(*self), f)
    }
}
impl DisplayWithSymbolTable for InlineObject {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        DisplayWithSymbolTable::fmt(&InlineData::from(*self), f, symbol_table)
    }
}

impl Eq for InlineObject {}
impl PartialEq for InlineObject {
    fn eq(&self, other: &Self) -> bool {
        InlineData::from(*self) == InlineData::from(*other)
    }
}
impl Hash for InlineObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        InlineData::from(*self).hash(state)
    }
}
impl OrdWithSymbolTable for InlineObject {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        OrdWithSymbolTable::cmp(&Data::from(*self), symbol_table, &Data::from(*other))
    }
}

impl TryFrom<InlineObject> for HeapObject {
    type Error = ();

    fn try_from(object: InlineObject) -> Result<Self, Self::Error> {
        match InlineData::from(object) {
            InlineData::Pointer(value) => Ok(value.get()),
            _ => Err(()),
        }
    }
}

#[enum_dispatch]
pub trait InlineObjectTrait: Copy + Debug + DisplayWithSymbolTable + Eq + Hash {
    fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
#[enum_dispatch(InlineObjectTrait)]
pub enum InlineData {
    Pointer(InlinePointer),
    Int(InlineInt),
    Builtin(InlineBuiltin),
    Tag(InlineTag),
    SendPort(InlineSendPort),
    ReceivePort(InlineReceivePort),
}
impl InlineData {
    fn channel_id(&self) -> Option<ChannelId> {
        match self {
            InlineData::SendPort(port) => Some(port.channel_id()),
            InlineData::ReceivePort(port) => Some(port.channel_id()),
            _ => None,
        }
    }
}

impl From<InlineObject> for InlineData {
    fn from(object: InlineObject) -> Self {
        let value = object.0.get();
        match value & InlineObject::KIND_MASK {
            InlineObject::KIND_POINTER => InlineData::Pointer(InlinePointer::new_unchecked(object)),
            InlineObject::KIND_INT => InlineData::Int(InlineInt::new_unchecked(object)),
            InlineObject::KIND_BUILTIN => InlineData::Builtin(InlineBuiltin::new_unchecked(object)),
            InlineObject::KIND_TAG => InlineData::Tag(InlineTag::new_unchecked(object)),
            InlineObject::KIND_SEND_PORT => {
                InlineData::SendPort(InlineSendPort::new_unchecked(object))
            }
            InlineObject::KIND_RECEIVE_PORT => {
                InlineData::ReceivePort(InlineReceivePort::new_unchecked(object))
            }
            _ => panic!("Unknown inline value type: {value:016x}"),
        }
    }
}

impl Debug for InlineData {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            InlineData::Pointer(value) => Debug::fmt(value, f),
            InlineData::Int(value) => Debug::fmt(value, f),
            InlineData::Builtin(value) => Debug::fmt(value, f),
            InlineData::Tag(value) => Debug::fmt(value, f),
            InlineData::SendPort(value) => Debug::fmt(value, f),
            InlineData::ReceivePort(value) => Debug::fmt(value, f),
        }
    }
}
impl DisplayWithSymbolTable for InlineData {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        match self {
            InlineData::Pointer(value) => DisplayWithSymbolTable::fmt(value, f, symbol_table),
            InlineData::Int(value) => Display::fmt(value, f),
            InlineData::Builtin(value) => Display::fmt(value, f),
            InlineData::Tag(value) => DisplayWithSymbolTable::fmt(value, f, symbol_table),
            InlineData::SendPort(value) => Display::fmt(value, f),
            InlineData::ReceivePort(value) => Display::fmt(value, f),
        }
    }
}

impl Deref for InlineData {
    type Target = InlineObject;

    fn deref(&self) -> &Self::Target {
        match self {
            InlineData::Pointer(value) => value,
            InlineData::Int(value) => value,
            InlineData::Builtin(value) => value,
            InlineData::Tag(value) => value,
            InlineData::SendPort(value) => value,
            InlineData::ReceivePort(value) => value,
        }
    }
}

#[extension_trait]
pub impl ToDebugText for InlineObject {
    fn to_debug_text(
        self,
        precendence: Precedence,
        max_length: MaxLength,
        symbol_table: &SymbolTable,
    ) -> String {
        format_value(self, precendence, max_length, &|value| {
            Some(match value.into() {
                Data::Int(int) => FormatValue::Int(int.get()),
                Data::Tag(tag) => FormatValue::Tag {
                    symbol: symbol_table.get(tag.symbol_id()),
                    value: tag.value(),
                },
                Data::Text(text) => FormatValue::Text(text.get()),
                Data::List(list) => FormatValue::List(list.items()),
                Data::Struct(struct_) => FormatValue::Struct(Cow::Owned(
                    struct_
                        .iter()
                        .map(|(_, key, value)| (key, value))
                        .collect_vec(),
                )),
                Data::HirId(_) => unreachable!(),
                Data::Function(_) => FormatValue::Function,
                Data::Builtin(_) => FormatValue::Function,
                Data::SendPort(_) => FormatValue::SendPort,
                Data::ReceivePort(_) => FormatValue::ReceivePort,
            })
        })
        .unwrap()
    }
}

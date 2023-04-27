use self::{
    builtin::InlineBuiltin,
    int::InlineInt,
    pointer::InlinePointer,
    port::{InlineReceivePort, InlineSendPort},
};
use super::{object_heap::HeapObject, Data, Heap, HirId};
use crate::{
    channel::ChannelId,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use enum_dispatch::enum_dispatch;
use extension_trait::extension_trait;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    ops::Deref,
};

pub(super) mod builtin;
pub(super) mod int;
pub(super) mod pointer;
pub(super) mod port;

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
pub struct InlineObject(u64);

impl InlineObject {
    pub const KIND_WIDTH: usize = 2;
    pub const KIND_MASK: u64 = 0b11;
    pub const KIND_POINTER: u64 = 0b00;
    pub const KIND_INT: u64 = 0b01;
    pub const KIND_PORT: u64 = 0b10;
    pub const KIND_PORT_SUBKIND_MASK: u64 = 0b100;
    pub const KIND_PORT_SUBKIND_SEND: u64 = 0b000;
    pub const KIND_PORT_SUBKIND_RECEIVE: u64 = 0b100;
    pub const KIND_BUILTIN: u64 = 0b11;

    pub fn new(value: u64) -> Self {
        Self(value)
    }
    pub fn raw_word(self) -> u64 {
        self.0
    }

    pub fn unwrap_hir_id(self) -> HirId {
        Data::from(self).unwrap_hir_id()
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
            it.dup_by(amount)
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

impl DebugDisplay for InlineObject {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        InlineData::from(*self).fmt(f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(InlineObject);

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
pub trait InlineObjectTrait: Copy + DebugDisplay + Eq + Hash {
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
    SendPort(InlineSendPort),
    ReceivePort(InlineReceivePort),
    Builtin(InlineBuiltin),
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
        let value = object.0;
        match value & InlineObject::KIND_MASK {
            InlineObject::KIND_POINTER => {
                debug_assert_eq!(value & 0b100, 0);
                InlineData::Pointer(InlinePointer::new_unchecked(object))
            }
            InlineObject::KIND_INT => InlineData::Int(InlineInt::new_unchecked(object)),
            InlineObject::KIND_PORT => match value & InlineObject::KIND_PORT_SUBKIND_MASK {
                InlineObject::KIND_PORT_SUBKIND_SEND => {
                    InlineData::SendPort(InlineSendPort::new_unchecked(object))
                }
                InlineObject::KIND_PORT_SUBKIND_RECEIVE => {
                    InlineData::ReceivePort(InlineReceivePort::new_unchecked(object))
                }
                _ => unreachable!(),
            },
            InlineObject::KIND_BUILTIN => InlineData::Builtin(InlineBuiltin::new_unchecked(object)),
            _ => unreachable!(),
        }
    }
}

impl DebugDisplay for InlineData {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            InlineData::Pointer(value) => value.fmt(f, is_debug),
            InlineData::Int(value) => value.fmt(f, is_debug),
            InlineData::SendPort(value) => value.fmt(f, is_debug),
            InlineData::ReceivePort(value) => value.fmt(f, is_debug),
            InlineData::Builtin(value) => value.fmt(f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(InlineData);

impl Deref for InlineData {
    type Target = InlineObject;

    fn deref(&self) -> &Self::Target {
        match self {
            InlineData::Pointer(value) => value,
            InlineData::Int(value) => value,
            InlineData::SendPort(value) => value,
            InlineData::ReceivePort(value) => value,
            InlineData::Builtin(value) => value,
        }
    }
}
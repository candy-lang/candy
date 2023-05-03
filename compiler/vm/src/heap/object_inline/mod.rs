use self::{
    builtin::InlineBuiltin,
    int::InlineInt,
    pointer::InlinePointer,
    port::{InlineReceivePort, InlineSendPort},
};
use super::{object_heap::HeapObject, Heap};
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
    marker::PhantomData,
    num::NonZeroU64,
    ops::Deref,
};

pub(super) mod builtin;
pub(super) mod int;
pub(super) mod pointer;
pub(super) mod port;

#[extension_trait]
pub impl<'h> InlineObjectSliceCloneToHeap<'h> for [InlineObject<'h>] {
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
pub struct InlineObject<'h> {
    value: NonZeroU64,
    phantom: PhantomData<&'h ()>,
}

impl<'h> InlineObject<'h> {
    pub const KIND_WIDTH: usize = 2;
    pub const KIND_MASK: u64 = 0b11;
    pub const KIND_POINTER: u64 = 0b00;
    pub const KIND_INT: u64 = 0b01;
    pub const KIND_PORT: u64 = 0b10;
    pub const KIND_PORT_SUBKIND_MASK: u64 = 0b100;
    pub const KIND_PORT_SUBKIND_SEND: u64 = 0b000;
    pub const KIND_PORT_SUBKIND_RECEIVE: u64 = 0b100;
    pub const KIND_BUILTIN: u64 = 0b11;

    pub fn new(value: NonZeroU64) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }
    pub fn raw_word(self) -> NonZeroU64 {
        self.value
    }

    // Reference Counting
    pub fn dup(self, heap: &'h mut Heap) {
        self.dup_by(heap, 1);
    }
    pub fn dup_by(self, heap: &'h mut Heap, amount: usize) {
        if let Some(channel) = InlineData::from(self).channel_id() {
            heap.dup_channel_by(channel, amount);
        };

        if let Ok(it) = HeapObject::try_from(self) {
            it.dup_by(amount)
        }
    }
    pub fn drop(self, heap: &'h mut Heap) {
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

impl DebugDisplay for InlineObject<'_> {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        InlineData::from(*self).fmt(f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(InlineObject<'_>);

impl Eq for InlineObject<'_> {}
impl PartialEq for InlineObject<'_> {
    fn eq(&self, other: &Self) -> bool {
        InlineData::from(*self) == InlineData::from(*other)
    }
}
impl Hash for InlineObject<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        InlineData::from(*self).hash(state)
    }
}

impl<'h> TryFrom<InlineObject<'h>> for HeapObject<'h> {
    type Error = ();

    fn try_from(object: InlineObject) -> Result<Self, Self::Error> {
        match InlineData::from(object) {
            InlineData::Pointer(value) => Ok(value.get()),
            _ => Err(()),
        }
    }
}

#[enum_dispatch]
pub trait InlineObjectTrait<'h>: Copy + DebugDisplay + Eq + Hash {
    fn clone_to_heap_with_mapping<'t>(
        self,
        heap: &'t mut Heap,
        address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) -> Self;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
#[enum_dispatch(InlineObjectTrait)]
pub enum InlineData<'h> {
    Pointer(InlinePointer<'h>),
    Int(InlineInt<'h>),
    SendPort(InlineSendPort<'h>),
    ReceivePort(InlineReceivePort<'h>),
    Builtin(InlineBuiltin<'h>),
}
impl InlineData<'_> {
    fn channel_id(&self) -> Option<ChannelId> {
        match self {
            InlineData::SendPort(port) => Some(port.channel_id()),
            InlineData::ReceivePort(port) => Some(port.channel_id()),
            _ => None,
        }
    }
}

impl<'h> From<InlineObject<'h>> for InlineData<'h> {
    fn from(object: InlineObject) -> Self {
        let value = object.value.get();
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

impl DebugDisplay for InlineData<'_> {
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
impl_debug_display_via_debugdisplay!(InlineData<'_>);

impl<'h> Deref for InlineData<'h> {
    type Target = InlineObject<'h>;

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

use super::{InlineObject, InlineObjectTrait};
use crate::{
    channel::ChannelId,
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::id::CountableId;
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::fmt::{self, Formatter};

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlinePort(InlineObject);

impl InlinePort {
    const CHANNEL_ID_SHIFT: usize = 3;

    pub fn create(channel_id: ChannelId, is_send: bool) -> InlineObject {
        let channel_id = channel_id.to_usize();
        debug_assert_eq!(
            (channel_id << Self::CHANNEL_ID_SHIFT) >> Self::CHANNEL_ID_SHIFT,
            channel_id,
            "Channel ID is too large.",
        );

        let subkind = if is_send {
            InlineObject::KIND_PORT_SUBKIND_SEND
        } else {
            InlineObject::KIND_PORT_SUBKIND_RECEIVE
        };
        InlineObject(
            InlineObject::KIND_PORT | subkind | ((channel_id as u64) << Self::CHANNEL_ID_SHIFT),
        )
    }

    pub fn channel_id(self) -> ChannelId {
        ChannelId::from_usize((self.0 .0 >> Self::CHANNEL_ID_SHIFT) as usize)
    }
}
impl From<InlinePort> for InlineObject {
    fn from(port: InlinePort) -> Self {
        port.0
    }
}

// Send Port

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlineSendPort(InlinePort);

impl InlineSendPort {
    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(channel_id: ChannelId) -> InlineObject {
        InlinePort::create(channel_id, true)
    }
}

impl DebugDisplay for InlineSendPort {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "send port for {:?}", self.channel_id())
    }
}
impl_debug_display_via_debugdisplay!(InlineSendPort);

impl InlineObjectTrait for InlineSendPort {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}

// Receive Port

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlineReceivePort(InlinePort);

impl InlineReceivePort {
    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(channel_id: ChannelId) -> InlineObject {
        InlinePort::create(channel_id, false)
    }
}

impl DebugDisplay for InlineReceivePort {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "receive port for {:?}", self.channel_id())
    }
}
impl_debug_display_via_debugdisplay!(InlineReceivePort);

impl InlineObjectTrait for InlineReceivePort {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}

use super::{InlineObject, InlineObjectTrait};
use crate::{
    channel::ChannelId,
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::id::CountableId;
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

#[derive(Clone, Copy, Deref)]
pub struct InlinePort(InlineObject);

impl InlinePort {
    const CHANNEL_ID_SHIFT: usize = 3;

    pub fn create(heap: &mut Heap, channel_id: ChannelId, is_send: bool) -> InlineObject {
        heap.notify_port_created(channel_id);
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
        let header_word =
            InlineObject::KIND_PORT | subkind | ((channel_id as u64) << Self::CHANNEL_ID_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        InlineObject(header_word)
    }

    pub fn channel_id(self) -> ChannelId {
        ChannelId::from_usize((self.raw_word().get() >> Self::CHANNEL_ID_SHIFT) as usize)
    }
}
impl From<InlinePort> for InlineObject {
    fn from(port: InlinePort) -> Self {
        port.0
    }
}

impl Eq for InlinePort {}
impl PartialEq for InlinePort {
    fn eq(&self, other: &Self) -> bool {
        self.channel_id() == other.channel_id()
    }
}
impl Hash for InlinePort {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.channel_id().hash(state)
    }
}
impl Ord for InlinePort {
    fn cmp(&self, other: &Self) -> Ordering {
        self.channel_id().cmp(&other.channel_id())
    }
}
impl PartialOrd for InlinePort {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Send Port

#[derive(Clone, Copy, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InlineSendPort(InlinePort);

impl InlineSendPort {
    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(heap: &mut Heap, channel_id: ChannelId) -> InlineObject {
        InlinePort::create(heap, channel_id, true)
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
        heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        heap.notify_port_created(self.channel_id());
        self
    }
}

// Receive Port

#[derive(Clone, Copy, Deref, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InlineReceivePort(InlinePort);

impl InlineReceivePort {
    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(heap: &mut Heap, channel_id: ChannelId) -> InlineObject {
        InlinePort::create(heap, channel_id, false)
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
        heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        heap.notify_port_created(self.channel_id());
        self
    }
}

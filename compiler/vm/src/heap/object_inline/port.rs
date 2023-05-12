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
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

#[derive(Clone, Copy, Deref)]
pub struct InlinePort<'h>(InlineObject<'h>);

impl<'h> InlinePort<'h> {
    const CHANNEL_ID_SHIFT: usize = 3;

    pub fn create(heap: &mut Heap<'h>, channel_id: ChannelId, is_send: bool) -> InlineObject<'h> {
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
        InlineObject::new(header_word)
    }

    pub fn channel_id(self) -> ChannelId {
        ChannelId::from_usize((self.raw_word().get() >> Self::CHANNEL_ID_SHIFT) as usize)
    }
}
impl<'h> From<InlinePort<'h>> for InlineObject<'h> {
    fn from(port: InlinePort<'h>) -> Self {
        port.0
    }
}

impl Eq for InlinePort<'_> {}
impl PartialEq for InlinePort<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.channel_id() == other.channel_id()
    }
}
impl Hash for InlinePort<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.channel_id().hash(state)
    }
}

// Send Port

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlineSendPort<'h>(InlinePort<'h>);

impl<'h> InlineSendPort<'h> {
    pub fn new_unchecked(object: InlineObject<'h>) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(heap: &mut Heap<'h>, channel_id: ChannelId) -> InlineObject<'h> {
        InlinePort::create(heap, channel_id, true)
    }
}

impl DebugDisplay for InlineSendPort<'_> {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "send port for {:?}", self.channel_id())
    }
}
impl_debug_display_via_debugdisplay!(InlineSendPort<'_>);

impl<'h> InlineObjectTrait<'h> for InlineSendPort<'h> {
    type Clone<'t> = InlineSendPort<'t>;

    fn clone_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        _address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) -> Self::Clone<'t> {
        heap.notify_port_created(self.channel_id());
        InlineSendPort(InlinePort(InlineObject::new(self.raw_word())))
    }
}

// Receive Port

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlineReceivePort<'h>(InlinePort<'h>);

impl<'h> InlineReceivePort<'h> {
    pub fn new_unchecked(object: InlineObject<'h>) -> Self {
        Self(InlinePort(object))
    }
    pub fn create(heap: &mut Heap<'h>, channel_id: ChannelId) -> InlineObject<'h> {
        InlinePort::create(heap, channel_id, false)
    }
}

impl DebugDisplay for InlineReceivePort<'_> {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "receive port for {:?}", self.channel_id())
    }
}
impl_debug_display_via_debugdisplay!(InlineReceivePort<'_>);

impl<'h> InlineObjectTrait<'h> for InlineReceivePort<'h> {
    type Clone<'t> = InlineReceivePort<'t>;

    fn clone_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        _address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) -> Self::Clone<'t> {
        heap.notify_port_created(self.channel_id());
        InlineReceivePort(InlinePort(InlineObject::new(self.raw_word())))
    }
}

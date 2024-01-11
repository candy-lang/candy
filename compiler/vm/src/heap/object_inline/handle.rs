use super::{InlineObject, InlineObjectTrait};
use crate::{
    handle_id::HandleId,
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
pub struct InlineHandle(InlineObject);

impl InlineHandle {
    const HANDLE_ID_SHIFT: usize = 32;
    const ARGUMENT_COUNT_SHIFT: usize = 4;

    #[must_use]
    pub const fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    #[must_use]
    pub fn create(heap: &mut Heap, handle_id: HandleId, argument_count: usize) -> Self {
        heap.notify_handle_created(handle_id);
        let handle_id = handle_id.to_usize();
        debug_assert_eq!(
            (handle_id << Self::HANDLE_ID_SHIFT) >> Self::HANDLE_ID_SHIFT,
            handle_id,
            "Handle ID is too large.",
        );
        let argument_count_shift_for_max_size = Self::HANDLE_ID_SHIFT + Self::ARGUMENT_COUNT_SHIFT;
        debug_assert_eq!(
            (argument_count << argument_count_shift_for_max_size)
                >> argument_count_shift_for_max_size,
            argument_count,
            "Handle accepts too many arguments.",
        );

        let header_word = InlineObject::KIND_HANDLE
            | ((handle_id as u64) << Self::HANDLE_ID_SHIFT)
            | ((argument_count as u64) << Self::ARGUMENT_COUNT_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Self(InlineObject(header_word))
    }

    #[must_use]
    pub fn handle_id(self) -> HandleId {
        HandleId::from_usize((self.raw_word().get() >> Self::HANDLE_ID_SHIFT) as usize)
    }

    #[must_use]
    pub fn argument_count(self) -> usize {
        ((self.raw_word().get() & 0xFFFF_FFFF) >> Self::ARGUMENT_COUNT_SHIFT) as usize
    }
}
impl From<InlineHandle> for InlineObject {
    fn from(handle: InlineHandle) -> Self {
        handle.0
    }
}

impl Eq for InlineHandle {}
impl PartialEq for InlineHandle {
    fn eq(&self, other: &Self) -> bool {
        self.handle_id() == other.handle_id()
    }
}
impl Hash for InlineHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle_id().hash(state);
    }
}
impl Ord for InlineHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.handle_id().cmp(&other.handle_id())
    }
}
impl PartialOrd for InlineHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl DebugDisplay for InlineHandle {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{:?}", self.handle_id())
    }
}
impl_debug_display_via_debugdisplay!(InlineHandle);

impl InlineObjectTrait for InlineHandle {
    fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        heap.notify_handle_created(self.handle_id());
        self
    }
}

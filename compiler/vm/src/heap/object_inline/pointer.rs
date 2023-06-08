use super::{InlineObject, InlineObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get, DebugDisplay},
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    num::NonZeroU64,
    ptr::NonNull,
};

#[derive(Clone, Copy, Deref)]
pub struct InlinePointer<'h>(InlineObject<'h>);
impl<'h> InlinePointer<'h> {
    pub fn new_unchecked(object: InlineObject<'h>) -> Self {
        Self(object)
    }

    pub fn get(self) -> HeapObject<'h> {
        let pointer = unsafe { NonNull::new_unchecked(self.raw_word().get() as *mut u64) };
        HeapObject::new(pointer)
    }
}

impl DebugDisplay for InlinePointer<'_> {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        self.get().fmt(f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(InlinePointer<'_>);

impl_eq_hash_ord_via_get!(InlinePointer<'_>);

impl<'h> From<HeapObject<'h>> for InlinePointer<'h> {
    fn from(value: HeapObject<'h>) -> Self {
        Self(value.into())
    }
}
impl<'h> From<HeapObject<'h>> for InlineObject<'h> {
    fn from(value: HeapObject<'h>) -> Self {
        let address = value.address().addr().get() as u64;
        debug_assert_eq!(address & Self::KIND_MASK, Self::KIND_POINTER);
        let address = unsafe { NonZeroU64::new_unchecked(address) };
        Self::new(address)
    }
}

impl<'h> InlineObjectTrait<'h> for InlinePointer<'h> {
    type Clone<'t> = InlinePointer<'t>;

    fn clone_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) -> Self::Clone<'t> {
        self.get()
            .clone_to_heap_with_mapping(heap, address_map)
            .into()
    }
}

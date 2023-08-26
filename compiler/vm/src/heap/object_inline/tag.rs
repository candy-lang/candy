use super::InlineObjectTrait;
use crate::{
    heap::{
        object_heap::{text::HeapText, HeapObject},
        Heap, InlineObject, Text,
    },
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
pub struct InlineTag(InlineObject);

impl InlineTag {
    const SYMBOL_POINTER_MASK: u64 = !0b111;

    pub const fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }
    pub fn new(symbol: Text) -> Self {
        let symbol_pointer = symbol.address().addr().get() as u64;
        debug_assert_eq!(
            symbol_pointer & Self::SYMBOL_POINTER_MASK,
            symbol_pointer,
            "Symbol pointer is invalid.",
        );
        let header_word = InlineObject::KIND_TAG | symbol_pointer;
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Self(InlineObject::new(header_word))
    }

    pub fn get(self) -> Text {
        let pointer = self.raw_word().get() & Self::SYMBOL_POINTER_MASK;
        let pointer = unsafe { NonNull::new_unchecked(pointer as *mut u64) };
        Text::from(HeapText::new_unchecked(HeapObject::new(pointer)))
    }

    pub fn dup_by(self, amount: usize) {
        self.get().dup_by(amount);
    }
    pub fn drop(self, heap: &mut Heap) {
        self.get().drop(heap);
    }
}

impl DebugDisplay for InlineTag {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "{}", self.get().get())
    }
}
impl_debug_display_via_debugdisplay!(InlineTag);

impl_eq_hash_ord_via_get!(InlineTag);

impl InlineObjectTrait for InlineTag {
    fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        let cloned = self.get().clone_to_heap_with_mapping(heap, address_map);
        Self::new(HeapText::new_unchecked(cloned).into())
    }
}

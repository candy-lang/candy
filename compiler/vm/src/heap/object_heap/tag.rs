use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject, Text},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ptr::NonNull,
    str,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapTag<'h>(HeapObject<'h>);

impl<'h> HeapTag<'h> {
    pub fn new_unchecked(object: HeapObject<'h>) -> Self {
        Self(object)
    }
    pub fn create(
        heap: &mut Heap<'h>,
        symbol: Text,
        value: impl Into<Option<InlineObject>>,
    ) -> Self {
        let value = value.into();
        let tag = Self(heap.allocate(HeapObject::KIND_TAG, 2 * HeapObject::WORD_SIZE));
        unsafe {
            *tag.symbol_pointer().as_mut() = symbol.into();
            *tag.value_pointer().as_mut() = value.map_or(0, |value| value.raw_word().get());
        };
        tag
    }

    fn symbol_pointer(self) -> NonNull<InlineObject<'h>> {
        self.content_word_pointer(0).cast()
    }
    pub fn symbol(self) -> Text<'h> {
        let symbol = unsafe { *self.symbol_pointer().as_ref() };
        symbol.try_into().unwrap()
    }

    fn value_pointer(self) -> NonNull<u64> {
        self.content_word_pointer(1)
    }
    pub fn has_value(self) -> bool {
        unsafe { *self.value_pointer().as_ref() != 0 }
    }
    pub fn value(self) -> Option<InlineObject<'h>> {
        let value = unsafe { *self.value_pointer().as_ref() };
        NonZeroU64::new(value).map(InlineObject::new)
    }
}

impl DebugDisplay for HeapTag<'_> {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "{}", self.symbol().get())?;

        if let Some(value) = self.value() {
            write!(f, " (")?;
            DebugDisplay::fmt(&value, f, is_debug)?;
            write!(f, ")")?;
        }
        Ok(())
    }
}
impl_debug_display_via_debugdisplay!(HeapTag<'_>);

impl Eq for HeapTag<'_> {}
impl PartialEq for HeapTag<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.symbol() == other.symbol() && self.value() == other.value()
    }
}

impl Hash for HeapTag<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol().hash(state);
        if let Some(value) = self.value() {
            value.hash(state);
        }
    }
}

impl Ord for HeapTag<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol()
            .cmp(&other.symbol())
            .then_with(|| self.value().cmp(&other.value()))
    }
}
impl PartialOrd for HeapTag<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

heap_object_impls!(HeapTag<'_>);

impl<'h> HeapObjectTrait<'h> for HeapTag<'h> {
    fn content_size(self) -> usize {
        2 * HeapObject::WORD_SIZE
    }

    fn clone_content_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        clone: HeapObject<'t>,
        address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) {
        let symbol = self.symbol().clone_to_heap_with_mapping(heap, address_map);
        let value = self
            .value()
            .map(|it| it.clone_to_heap_with_mapping(heap, address_map));
        let clone = HeapTag(clone);
        unsafe {
            *clone.symbol_pointer().as_mut() = symbol.into();
            *clone.value_pointer().as_mut() = value.map_or(0, |it| it.raw_word().get());
        };
    }

    fn drop_children(self, heap: &mut Heap<'h>) {
        self.symbol().drop(heap);
        if let Some(value) = self.value() {
            value.drop(heap);
        }
    }

    fn deallocate_external_stuff(self) {}
}

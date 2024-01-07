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
pub struct HeapTag(HeapObject);

impl HeapTag {
    #[must_use]
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    #[must_use]
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol: Text,
        value: impl Into<InlineObject>,
    ) -> Self {
        let value = value.into();
        let tag = Self(heap.allocate(
            HeapObject::KIND_TAG,
            is_reference_counted,
            0,
            2 * HeapObject::WORD_SIZE,
        ));
        unsafe {
            *tag.symbol_pointer().as_mut() = symbol.into();
            *tag.value_pointer().as_mut() = value.raw_word().get();
        };
        tag
    }

    #[must_use]
    fn symbol_pointer(self) -> NonNull<InlineObject> {
        self.content_word_pointer(0).cast()
    }
    #[must_use]
    pub fn symbol(self) -> Text {
        let symbol = unsafe { *self.symbol_pointer().as_ref() };
        symbol.try_into().unwrap()
    }

    #[must_use]
    fn value_pointer(self) -> NonNull<u64> {
        self.content_word_pointer(1)
    }
    #[must_use]
    pub fn value(self) -> InlineObject {
        let value = unsafe { *self.value_pointer().as_ref() };
        InlineObject::new(NonZeroU64::new(value).unwrap())
    }
}

impl DebugDisplay for HeapTag {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "{}", self.symbol().get())?;

        write!(f, " (")?;
        DebugDisplay::fmt(&self.value(), f, is_debug)?;
        write!(f, ")")
    }
}
impl_debug_display_via_debugdisplay!(HeapTag);

impl Eq for HeapTag {}
impl PartialEq for HeapTag {
    fn eq(&self, other: &Self) -> bool {
        self.symbol() == other.symbol() && self.value() == other.value()
    }
}

impl Hash for HeapTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol().hash(state);
        self.value().hash(state);
    }
}

impl Ord for HeapTag {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol()
            .cmp(&other.symbol())
            .then_with(|| self.value().cmp(&other.value()))
    }
}
impl PartialOrd for HeapTag {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

heap_object_impls!(HeapTag);

impl HeapObjectTrait for HeapTag {
    fn content_size(self) -> usize {
        2 * HeapObject::WORD_SIZE
    }

    fn clone_content_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        clone: HeapObject,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let symbol = self.symbol().clone_to_heap_with_mapping(heap, address_map);
        let value = self.value().clone_to_heap_with_mapping(heap, address_map);
        let clone = Self(clone);
        unsafe {
            *clone.symbol_pointer().as_mut() = symbol.into();
            *clone.value_pointer().as_mut() = value.raw_word().get();
        };
    }

    fn drop_children(self, heap: &mut Heap) {
        self.symbol().drop(heap);
        self.value().drop(heap);
    }

    fn deallocate_external_stuff(self) {}
}

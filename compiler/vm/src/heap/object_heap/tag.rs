use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::heap::{
    object_heap::HeapObject,
    symbol_table::{DisplayWithSymbolTable, OrdWithSymbolTable},
    Heap, InlineObject, SymbolId, SymbolTable, Tag,
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ptr::NonNull,
    str,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapTag(HeapObject);

impl HeapTag {
    const SYMBOL_ID_SHIFT: usize = 4;

    pub fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        symbol_id: SymbolId,
        value: impl Into<Option<InlineObject>>,
    ) -> Self {
        let symbol_id = symbol_id.value();
        debug_assert_eq!(
            (symbol_id << Self::SYMBOL_ID_SHIFT) >> Self::SYMBOL_ID_SHIFT,
            symbol_id,
            "Symbol ID is too large.",
        );

        let value = value.into();
        let tag = Self(heap.allocate(
            HeapObject::KIND_TAG,
            is_reference_counted,
            (symbol_id as u64) << Self::SYMBOL_ID_SHIFT,
            2 * HeapObject::WORD_SIZE,
        ));
        unsafe {
            *tag.value_pointer().as_mut() = value.map_or(0, |value| value.raw_word().get());
        };
        tag
    }

    pub fn symbol_id(self) -> SymbolId {
        let header_word = self.header_word();
        SymbolId::from((header_word >> Self::SYMBOL_ID_SHIFT) as usize)
    }

    fn value_pointer(self) -> NonNull<u64> {
        self.content_word_pointer(0)
    }
    pub fn has_value(self) -> bool {
        unsafe { *self.value_pointer().as_ref() != 0 }
    }
    pub fn value(self) -> Option<InlineObject> {
        let value = unsafe { *self.value_pointer().as_ref() };
        NonZeroU64::new(value).map(InlineObject::new)
    }

    pub fn without_value(self, heap: &mut Heap) -> Tag {
        Tag::create(heap, true, self.symbol_id(), None)
    }
}

impl Debug for HeapTag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "<symbol-id {}>", self.symbol_id().value())?;

        if let Some(value) = self.value() {
            write!(f, " ({value:?})")?;
        }
        Ok(())
    }
}
impl DisplayWithSymbolTable for HeapTag {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "{}", symbol_table.get(self.symbol_id()))?;

        if let Some(value) = self.value() {
            write!(
                f,
                " ({})",
                DisplayWithSymbolTable::to_string(&value, symbol_table),
            )?;
        }
        Ok(())
    }
}

impl Eq for HeapTag {}
impl PartialEq for HeapTag {
    fn eq(&self, other: &Self) -> bool {
        self.symbol_id() == other.symbol_id() && self.value() == other.value()
    }
}

impl Hash for HeapTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol_id().hash(state);
        if let Some(value) = self.value() {
            value.hash(state);
        }
    }
}

impl OrdWithSymbolTable for HeapTag {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        symbol_table
            .get(self.symbol_id())
            .cmp(symbol_table.get(other.symbol_id()))
            .then_with(|| self.value().cmp(symbol_table, &other.value()))
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
        let value = self
            .value()
            .map(|it| it.clone_to_heap_with_mapping(heap, address_map));
        let clone = Self(clone);
        unsafe {
            *clone.value_pointer().as_mut() = value.map_or(0, |it| it.raw_word().get());
        };
    }

    fn drop_children(self, heap: &mut Heap) {
        if let Some(value) = self.value() {
            value.drop(heap);
        }
    }

    fn deallocate_external_stuff(self) {}
}

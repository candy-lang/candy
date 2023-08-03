use super::{InlineObject, InlineObjectTrait};
use crate::heap::{
    object_heap::HeapObject, DisplayWithSymbolTable, Heap, OrdWithSymbolTable, SymbolTable,
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ptr::NonNull,
};

#[derive(Clone, Copy, Deref)]
pub struct InlinePointer(InlineObject);
impl InlinePointer {
    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    pub fn get(self) -> HeapObject {
        let pointer = unsafe { NonNull::new_unchecked(self.raw_word().get() as *mut u64) };
        HeapObject::new(pointer)
    }
}

impl Debug for InlinePointer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.get())
    }
}
impl DisplayWithSymbolTable for InlinePointer {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        DisplayWithSymbolTable::fmt(&self.get(), f, symbol_table)
    }
}

impl Eq for InlinePointer {}
impl PartialEq for InlinePointer {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl Hash for InlinePointer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get().hash(state)
    }
}

impl OrdWithSymbolTable for InlinePointer {
    fn cmp(&self, symbol_table: &SymbolTable, other: &Self) -> Ordering {
        self.get().cmp(symbol_table, &other.get())
    }
}

impl From<HeapObject> for InlinePointer {
    fn from(value: HeapObject) -> Self {
        Self(value.into())
    }
}
impl From<HeapObject> for InlineObject {
    fn from(value: HeapObject) -> Self {
        let address = value.address().addr().get() as u64;
        debug_assert_eq!(address & Self::KIND_MASK, Self::KIND_POINTER);
        let address = unsafe { NonZeroU64::new_unchecked(address) };
        Self(address)
    }
}

impl InlineObjectTrait for InlinePointer {
    fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self.get()
            .clone_to_heap_with_mapping(heap, address_map)
            .into()
    }
}

use super::InlineObjectTrait;
use crate::{
    heap::{
        object_heap::HeapObject, symbol_table::impl_ops_with_symbol_table_via_ops,
        DisplayWithSymbolTable, Heap, InlineObject, SymbolId, SymbolTable,
    },
    utils::impl_eq_hash_ord_via_get,
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Debug, Formatter},
    num::NonZeroU64,
};

#[derive(Clone, Copy, Deref)]
pub struct InlineTag(InlineObject);

impl InlineTag {
    const SYMBOL_ID_SHIFT: usize = 3;

    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    pub fn get(self) -> SymbolId {
        SymbolId::from((self.raw_word().get() >> Self::SYMBOL_ID_SHIFT) as usize)
    }
}

impl Debug for InlineTag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "<symbol-id {}>", self.get().value())
    }
}
impl DisplayWithSymbolTable for InlineTag {
    fn fmt(&self, f: &mut Formatter, symbol_table: &SymbolTable) -> fmt::Result {
        // We can always use the display formatter since the symbol has a constrained charset.
        write!(f, "{}", symbol_table.get(self.get()))
    }
}

impl_eq_hash_ord_via_get!(InlineTag);

impl From<SymbolId> for InlineObject {
    fn from(symbol_id: SymbolId) -> Self {
        *InlineTag::from(symbol_id)
    }
}
impl From<SymbolId> for InlineTag {
    fn from(symbol_id: SymbolId) -> Self {
        let symbol_id = symbol_id.value();
        debug_assert_eq!(
            (symbol_id << Self::SYMBOL_ID_SHIFT) >> Self::SYMBOL_ID_SHIFT,
            symbol_id,
            "Symbol ID is too large.",
        );
        let header_word = InlineObject::KIND_TAG | ((symbol_id as u64) << Self::SYMBOL_ID_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Self(InlineObject::new(header_word))
    }
}

impl InlineObjectTrait for InlineTag {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}

impl_ops_with_symbol_table_via_ops!(InlineTag);

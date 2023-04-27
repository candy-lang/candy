use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_via_get, DebugDisplay},
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    ptr::{self, NonNull},
    slice, str,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapSymbol(HeapObject);

impl HeapSymbol {
    const LEN_SHIFT: usize = 3;

    pub fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, value: &str) -> Self {
        let len = value.len();
        assert_eq!(
            (len << Self::LEN_SHIFT) >> Self::LEN_SHIFT,
            len,
            "Symbol is too long.",
        );
        let symbol = Self(heap.allocate(
            HeapObject::KIND_SYMBOL | ((len as u64) << Self::LEN_SHIFT),
            len,
        ));
        unsafe { ptr::copy_nonoverlapping(value.as_ptr(), symbol.symbol_pointer().as_ptr(), len) };
        symbol
    }

    pub fn len(self) -> usize {
        (self.header_word() >> Self::LEN_SHIFT) as usize
    }
    fn symbol_pointer(self) -> NonNull<u8> {
        self.content_word_pointer(0).cast()
    }
    pub fn get<'a>(self) -> &'a str {
        let pointer = self.symbol_pointer().as_ptr();
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(pointer, self.len())) }
    }
}

impl DebugDisplay for HeapSymbol {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(HeapSymbol);

impl_eq_hash_via_get!(HeapSymbol);

heap_object_impls!(HeapSymbol);

impl HeapObjectTrait for HeapSymbol {
    fn content_size(self) -> usize {
        self.len()
    }

    fn clone_content_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        clone: HeapObject,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        unsafe {
            ptr::copy_nonoverlapping(
                self.symbol_pointer().as_ptr(),
                clone.symbol_pointer().as_ptr(),
                self.len(),
            )
        };
    }

    fn drop_children(self, _heap: &mut Heap) {}
}

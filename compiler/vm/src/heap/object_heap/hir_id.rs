use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_via_get, DebugDisplay},
};
use candy_frontend::hir::Id;
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    mem, ptr,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapHirId(HeapObject);

impl HeapHirId {
    pub fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, value: Id) -> Self {
        let object = heap.allocate(HeapObject::KIND_HIR_ID, mem::size_of::<Id>());
        unsafe { ptr::write(object.content_word_pointer(0).cast().as_ptr(), value) };
        HeapHirId(object)
    }

    pub fn get<'a>(self) -> &'a Id {
        unsafe { &*self.content_word_pointer(0).cast().as_ptr() }
    }
}

impl DebugDisplay for HeapHirId {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(HeapHirId);

impl_eq_hash_via_get!(HeapHirId);

heap_object_impls!(HeapHirId);

impl HeapObjectTrait for HeapHirId {
    fn content_size(self) -> usize {
        mem::size_of::<Id>()
    }

    fn clone_content_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        clone: HeapObject,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        let value = self.get().to_owned();
        unsafe { ptr::write(clone.content_word_pointer(0).cast().as_ptr(), value) };
    }

    fn drop_children(self, _heap: &mut Heap) {
        unsafe { ptr::drop_in_place(self.content_word_pointer(0).cast::<Id>().as_ptr()) };
    }
}

use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap},
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get, DebugDisplay},
};
use candy_frontend::hir::Id;
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    mem,
    ptr::{self, NonNull},
};

#[derive(Clone, Copy, Deref)]
pub struct HeapHirId(HeapObject);

impl HeapHirId {
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: Id) -> Self {
        let id = Self(heap.allocate(
            HeapObject::KIND_HIR_ID,
            is_reference_counted,
            0,
            mem::size_of::<Id>(),
        ));
        unsafe { ptr::write(id.id_pointer().as_ptr(), value) };
        id
    }

    fn id_pointer(self) -> NonNull<Id> {
        self.content_word_pointer(0).cast()
    }
    pub fn get<'a>(self) -> &'a Id {
        unsafe { &*self.id_pointer().as_ptr() }
    }
}

impl DebugDisplay for HeapHirId {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(HeapHirId);

impl_eq_hash_ord_via_get!(HeapHirId);

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
        let value = self.get().clone();
        unsafe { ptr::write(clone.id_pointer().as_ptr(), value) };
    }

    fn drop_children(self, _heap: &mut Heap) {}

    fn deallocate_external_stuff(self) {
        unsafe { ptr::drop_in_place(self.id_pointer().as_ptr()) };
    }
}

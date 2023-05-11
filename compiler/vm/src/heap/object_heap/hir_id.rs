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
    mem,
    ptr::{self, NonNull},
};

#[derive(Clone, Copy, Deref)]
pub struct HeapHirId<'h>(HeapObject<'h>);

impl<'h> HeapHirId<'h> {
    pub fn new_unchecked(object: HeapObject<'h>) -> Self {
        Self(object)
    }
    pub fn create(heap: &'h mut Heap, value: Id) -> Self {
        let id = HeapHirId(heap.allocate(HeapObject::KIND_HIR_ID, mem::size_of::<Id>()));
        unsafe { ptr::write(id.id_pointer().as_ptr(), value) };
        id
    }

    fn id_pointer(self) -> NonNull<Id> {
        self.content_word_pointer(0).cast()
    }
    pub fn get(self) -> &'h Id {
        unsafe { &*self.id_pointer().as_ptr() }
    }
}

impl DebugDisplay for HeapHirId<'_> {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(HeapHirId<'_>);

impl_eq_hash_via_get!(HeapHirId<'_>);

heap_object_impls!(HeapHirId<'h>);

impl<'h> HeapObjectTrait<'h> for HeapHirId<'h> {
    fn content_size(self) -> usize {
        mem::size_of::<Id>()
    }

    fn clone_content_to_heap_with_mapping<'t>(
        self,
        _heap: &'t mut Heap,
        clone: HeapObject<'t>,
        _address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) {
        let clone = Self(clone);
        let value = self.get().to_owned();
        unsafe { ptr::write(clone.id_pointer().as_ptr(), value) };
    }

    fn drop_children(self, _heap: &'h mut Heap) {}

    fn deallocate_external_stuff(self) {
        unsafe { ptr::drop_in_place(self.id_pointer().as_ptr()) };
    }
}

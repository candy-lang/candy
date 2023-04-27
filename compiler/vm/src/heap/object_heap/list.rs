use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use derive_more::Deref;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    ptr::{self, NonNull},
    slice,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapList(HeapObject);

impl HeapList {
    const LEN_SHIFT: usize = 3;

    pub fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, value: &[InlineObject]) -> Self {
        let len = value.len();
        let list = Self::create_uninitialized(heap, len);
        unsafe { ptr::copy_nonoverlapping(value.as_ptr(), list.items_pointer().as_ptr(), len) };
        list
    }
    fn create_uninitialized(heap: &mut Heap, len: usize) -> Self {
        assert_eq!(
            (len << Self::LEN_SHIFT) >> Self::LEN_SHIFT,
            len,
            "List is too long.",
        );
        Self(heap.allocate(
            HeapObject::KIND_LIST | ((len as u64) << Self::LEN_SHIFT),
            len * HeapObject::WORD_SIZE,
        ))
    }

    pub fn len(self) -> usize {
        (self.header_word() >> Self::LEN_SHIFT) as usize
    }
    pub fn get(self, index: usize) -> InlineObject {
        debug_assert!(index < self.len());
        InlineObject::new(self.unsafe_get_content_word(index))
    }
    fn items_pointer(self) -> NonNull<InlineObject> {
        self.content_word_pointer(0).cast()
    }
    pub fn items<'a>(self) -> &'a [InlineObject] {
        unsafe {
            let pointer = self.items_pointer().as_ref();
            slice::from_raw_parts(pointer, self.len())
        }
    }
    #[must_use]
    pub fn insert(self, heap: &mut Heap, index: usize, value: InlineObject) -> Self {
        assert!(index <= self.len());

        let len = self.len() + 1;
        let new_list = Self::create_uninitialized(heap, len);
        unsafe {
            ptr::copy_nonoverlapping(
                self.content_word_pointer(0).as_ptr(),
                new_list.content_word_pointer(0).as_ptr(),
                index,
            );
            ptr::write(new_list.content_word_pointer(index).cast().as_ptr(), value);
            ptr::copy_nonoverlapping(
                self.content_word_pointer(index).as_ptr(),
                new_list.content_word_pointer(index + 1).as_ptr(),
                self.len() - index,
            );
        }
        new_list
    }
    #[must_use]
    pub fn remove(self, heap: &mut Heap, index: usize) -> Self {
        assert!(index < self.len());

        let len = self.len() - 1;
        let new_list = Self::create_uninitialized(heap, len);
        unsafe {
            ptr::copy_nonoverlapping(
                self.content_word_pointer(0).as_ptr(),
                new_list.content_word_pointer(0).as_ptr(),
                index,
            );
            ptr::copy_nonoverlapping(
                self.content_word_pointer(index + 1).as_ptr(),
                new_list.content_word_pointer(index).as_ptr(),
                self.len() - index - 1,
            );
        }
        new_list
    }
    #[must_use]
    pub fn replace(self, heap: &mut Heap, index: usize, value: InlineObject) -> Self {
        assert!(index < self.len());

        let new_list = Self::create(heap, self.items());
        unsafe { ptr::write(new_list.content_word_pointer(index).cast().as_ptr(), value) };
        new_list
    }
}

impl DebugDisplay for HeapList {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        let items = self.items();
        write!(
            f,
            "({})",
            if items.is_empty() {
                ",".to_owned()
            } else {
                items
                    .iter()
                    .map(|item| DebugDisplay::to_string(item, is_debug))
                    .join(", ")
            },
        )
    }
}
impl_debug_display_via_debugdisplay!(HeapList);

impl Eq for HeapList {}
impl PartialEq for HeapList {
    fn eq(&self, other: &Self) -> bool {
        self.items() == other.items()
    }
}

impl Hash for HeapList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.items().hash(state)
    }
}

heap_object_impls!(HeapList);

impl HeapObjectTrait for HeapList {
    fn content_size(self) -> usize {
        self.len() * HeapObject::WORD_SIZE
    }

    fn clone_content_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        clone: HeapObject,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        for (index, &item) in self.items().iter().enumerate() {
            clone.unsafe_set_content_word(
                index,
                item.clone_to_heap_with_mapping(heap, address_map)
                    .raw_word(),
            );
        }
    }

    fn drop_children(self, heap: &mut Heap) {
        for item in self.items() {
            item.drop(heap)
        }
    }
}
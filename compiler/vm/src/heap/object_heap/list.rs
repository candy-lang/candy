use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ptr::{self, NonNull},
    slice,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapList(HeapObject);

impl HeapList {
    const LEN_SHIFT: usize = 4;

    #[must_use]
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    #[must_use]
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: &[InlineObject]) -> Self {
        let len = value.len();
        let list = Self::create_uninitialized(heap, is_reference_counted, len);
        unsafe { ptr::copy_nonoverlapping(value.as_ptr(), list.items_pointer().as_ptr(), len) };
        list
    }
    #[must_use]
    fn create_uninitialized(heap: &mut Heap, is_reference_counted: bool, len: usize) -> Self {
        debug_assert_eq!(
            (len << Self::LEN_SHIFT) >> Self::LEN_SHIFT,
            len,
            "List is too long.",
        );
        Self(heap.allocate(
            HeapObject::KIND_LIST,
            is_reference_counted,
            (len as u64) << Self::LEN_SHIFT,
            len * HeapObject::WORD_SIZE,
        ))
    }

    #[must_use]
    pub fn len(self) -> usize {
        (self.header_word() >> Self::LEN_SHIFT) as usize
    }
    #[must_use]
    pub fn get(self, index: usize) -> InlineObject {
        debug_assert!(index < self.len());
        let word = self.unsafe_get_content_word(index);
        let word = unsafe { NonZeroU64::new_unchecked(word) };
        InlineObject::new(word)
    }
    #[must_use]
    fn items_pointer(self) -> NonNull<InlineObject> {
        self.content_word_pointer(0).cast()
    }
    #[must_use]
    pub fn items<'a>(self) -> &'a [InlineObject] {
        unsafe {
            let pointer = self.items_pointer().as_ref();
            slice::from_raw_parts(pointer, self.len())
        }
    }
    #[must_use]
    pub fn insert(self, heap: &mut Heap, index: usize, value: InlineObject) -> Self {
        debug_assert!(index <= self.len());

        let len = self.len() + 1;
        let new_list = Self::create_uninitialized(heap, true, len);
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
        debug_assert!(index < self.len());

        let len = self.len() - 1;
        let new_list = Self::create_uninitialized(heap, true, len);
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
        debug_assert!(index < self.len());

        let new_list = Self::create(heap, true, self.items());
        unsafe { ptr::write(new_list.content_word_pointer(index).cast().as_ptr(), value) };
        new_list
    }
}

impl DebugDisplay for HeapList {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        let items = self.items();
        write!(f, "(")?;
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            DebugDisplay::fmt(item, f, is_debug)?;
        }
        if self.len() <= 1 {
            write!(f, ",")?;
        }
        write!(f, ")")
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
        self.items().hash(state);
    }
}

impl Ord for HeapList {
    fn cmp(&self, other: &Self) -> Ordering {
        self.items().cmp(other.items())
    }
}
impl PartialOrd for HeapList {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
                    .raw_word()
                    .get(),
            );
        }
    }

    fn drop_children(self, heap: &mut Heap) {
        for item in self.items() {
            item.drop(heap);
        }
    }

    fn deallocate_external_stuff(self) {}
}

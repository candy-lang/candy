use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use derive_more::Deref;
use itertools::{izip, Itertools};
use rustc_hash::{FxHashMap, FxHasher};
use std::{
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    ptr, slice,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapStruct(HeapObject);

impl HeapStruct {
    const LEN_SHIFT: usize = 3;

    pub fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, value: &FxHashMap<InlineObject, InlineObject>) -> Self {
        let len = value.len();
        assert_eq!(
            (len << Self::LEN_SHIFT) >> Self::LEN_SHIFT,
            len,
            "Struct is too long.",
        );
        let entries = value
            .iter()
            // PERF: Reuse hashes from the map.
            .map(|(&key, &value)| (Self::do_hash(key), key, value))
            .sorted_by_key(|(hash, _, _)| *hash);
        let struct_ = Self::create_uninitialized(heap, len);
        unsafe {
            for (index, (hash, key, value)) in entries.enumerate() {
                *struct_.content_word_pointer(index).as_ptr() = hash;
                *struct_.content_word_pointer(index + len).cast().as_ptr() = key;
                *struct_
                    .content_word_pointer(index + 2 * len)
                    .cast()
                    .as_ptr() = value;
            }
        };
        struct_
    }
    fn create_uninitialized(heap: &mut Heap, len: usize) -> Self {
        assert_eq!(
            (len << Self::LEN_SHIFT) >> Self::LEN_SHIFT,
            len,
            "Struct is too long.",
        );
        Self(heap.allocate(
            HeapObject::KIND_STRUCT | ((len as u64) << Self::LEN_SHIFT),
            3 * len * HeapObject::WORD_SIZE,
        ))
    }

    pub fn len(self) -> usize {
        (self.header_word() >> Self::LEN_SHIFT) as usize
    }
    pub fn hashes<'a>(self) -> &'a [u64] {
        self.items(0)
    }
    pub fn keys<'a>(self) -> &'a [InlineObject] {
        self.items(1)
    }
    pub fn values<'a>(self) -> &'a [InlineObject] {
        self.items(2)
    }
    pub fn iter<'a>(self) -> impl Iterator<Item = (u64, InlineObject, InlineObject)> + 'a {
        izip!(
            self.hashes().iter().copied(),
            self.keys().iter().copied(),
            self.values().iter().copied(),
        )
    }
    fn items<'a, T>(self, items_index: usize) -> &'a [T] {
        let len = self.len();
        unsafe {
            slice::from_raw_parts(
                self.content_word_pointer(items_index * len).cast().as_ptr(),
                len,
            )
        }
    }

    pub fn contains(self, key: InlineObject) -> bool {
        self.index_of_key(key, Self::do_hash(key)).is_ok()
    }
    pub fn get(self, key: impl Into<InlineObject>) -> Option<InlineObject> {
        let key = key.into();
        self.index_of_key(key, Self::do_hash(key))
            .ok()
            .map(|index| self.values()[index])
    }
    #[must_use]
    pub fn insert(self, heap: &mut Heap, key: InlineObject, value: InlineObject) -> Self {
        let hash = Self::do_hash(key);
        match self.index_of_key(key, hash) {
            Ok(index) => {
                let struct_ = Self::create_uninitialized(heap, self.len());
                unsafe {
                    ptr::copy_nonoverlapping(
                        self.content_word_pointer(0).as_ptr(),
                        struct_.content_word_pointer(0).as_ptr(),
                        3 * self.len(),
                    );
                    ptr::write(
                        struct_
                            .content_word_pointer(2 * self.len() + index)
                            .cast()
                            .as_ptr(),
                        value,
                    );
                }
                struct_
            }
            Err(index) => {
                let struct_ = Self::create_uninitialized(heap, self.len() + 1);
                // PERF: Merge consecutive copies.
                self.insert_into_items(struct_, 0, index, hash);
                self.insert_into_items(struct_, 1, index, key);
                self.insert_into_items(struct_, 2, index, value);
                struct_
            }
        }
    }
    fn insert_into_items<T>(self, other: Self, items_index: usize, index: usize, item: T) {
        let len = self.len();
        unsafe {
            ptr::copy_nonoverlapping(
                self.content_word_pointer(items_index * len).as_ptr(),
                other.content_word_pointer(items_index * len).as_ptr(),
                index,
            );
            *other
                .content_word_pointer(items_index * len + index)
                .cast()
                .as_ptr() = item;
            ptr::copy_nonoverlapping(
                self.content_word_pointer(items_index * len + index)
                    .as_ptr(),
                other
                    .content_word_pointer(items_index * len + index + 1)
                    .as_ptr(),
                len - index,
            );
        }
    }

    /// If the struct contains the key, returns the index of its field.
    /// Otherwise, returns the index of where the key would be inserted.
    fn index_of_key(self, key: InlineObject, key_hash: u64) -> Result<usize, usize> {
        let hashes = self.hashes();
        let keys = self.keys();
        let index_of_first_hash_occurrence =
            hashes.partition_point(|existing_hash| *existing_hash < key_hash);
        hashes[index_of_first_hash_occurrence..]
            .iter()
            .enumerate()
            .take_while(|(_, &existing_hash)| existing_hash == key_hash)
            .map(|(index, _)| index_of_first_hash_occurrence + index)
            .map(|index| (index, keys[index]))
            .find(|(_, existing_key)| *existing_key == key)
            .map(|(index, _)| index)
            .ok_or(index_of_first_hash_occurrence)
    }

    fn do_hash(key: InlineObject) -> u64 {
        let mut state = FxHasher::default();
        key.hash(&mut state);
        state.finish()
    }
}

impl DebugDisplay for HeapStruct {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        let keys = self.keys();
        if keys.is_empty() {
            write!(f, "[]")
        } else {
            let values = self.values();
            if is_debug {
                let hashes = self.hashes();
                write!(
                    f,
                    "[{}]",
                    izip!(hashes, keys, values)
                        .map(|(hash, key, value)| {
                            (
                                format!("{:016X}", hash),
                                DebugDisplay::to_string(key, is_debug),
                                DebugDisplay::to_string(value, is_debug),
                            )
                        })
                        .map(|(hash, key, value)| format!("{hash} → {key}: {value}"))
                        .join(", ")
                )
            } else {
                write!(
                    f,
                    "[{}]",
                    keys.iter()
                        .zip(values.iter())
                        .map(|(key, value)| (
                            DebugDisplay::to_string(key, is_debug),
                            DebugDisplay::to_string(value, is_debug)
                        ))
                        .sorted_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b))
                        .map(|(key, value)| format!("{key}: {value}"))
                        .join(", ")
                )
            }
        }
    }
}
impl_debug_display_via_debugdisplay!(HeapStruct);

impl Eq for HeapStruct {}
impl PartialEq for HeapStruct {
    fn eq(&self, other: &Self) -> bool {
        self.hashes() == other.hashes()
            && self.values() == other.values()
            && self.keys() == other.keys()
    }
}
impl Hash for HeapStruct {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hashes().hash(state);
        self.values().hash(state);
    }
}

heap_object_impls!(HeapStruct);

impl HeapObjectTrait for HeapStruct {
    fn content_size(self) -> usize {
        3 * self.len() * HeapObject::WORD_SIZE
    }

    fn clone_content_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        clone: HeapObject,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        unsafe {
            ptr::copy_nonoverlapping(
                self.content_word_pointer(0).as_ptr(),
                clone.content_word_pointer(0).as_ptr(),
                self.len(),
            )
        };
        for (index, &key) in self.keys().iter().enumerate() {
            clone.unsafe_set_content_word(
                index,
                key.clone_to_heap_with_mapping(heap, address_map)
                    .raw_word()
                    .get(),
            );
        }
        for (index, &value) in self.values().iter().enumerate() {
            clone.unsafe_set_content_word(
                index,
                value
                    .clone_to_heap_with_mapping(heap, address_map)
                    .raw_word()
                    .get(),
            );
        }
    }

    fn drop_children(self, heap: &mut Heap) {
        for key in self.keys() {
            key.drop(heap);
        }
        for value in self.values() {
            value.drop(heap);
        }
    }
}

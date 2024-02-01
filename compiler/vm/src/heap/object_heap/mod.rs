use self::{
    function::HeapFunction, hir_id::HeapHirId, int::HeapInt, list::HeapList, struct_::HeapStruct,
    tag::HeapTag, text::HeapText,
};
use super::{Data, Heap};
use crate::{
    heap::DEBUG_ALLOCATIONS,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use enum_dispatch::enum_dispatch;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    collections::hash_map,
    fmt::{self, Formatter, Pointer},
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    ops::{Deref, Range},
    ptr::NonNull,
};
use tracing::debug;

pub(super) mod function;
pub(super) mod hir_id;
pub(super) mod int;
pub(super) mod list;
pub(super) mod struct_;
pub(super) mod tag;
pub(super) mod text;
mod utils;

const TRACE: bool = false;
macro_rules! trace {
    ($format_string:tt, $($args:expr,)+) => {
        if TRACE {
            tracing::trace!($format_string, $($args),+)
        }
    };
    ($format_string:tt, $($args:expr),+) => {
        if TRACE {
            tracing::trace!($format_string, $($args),+)
        }
    };
    ($format_string:tt) => {
        if TRACE {
            tracing::trace!($format_string)
        }
    };
}

#[derive(Clone, Copy)]
pub struct HeapObject(NonNull<u64>);
impl HeapObject {
    pub const WORD_SIZE: usize = 8;

    pub const KIND_MASK: u64 = 0b111;
    const KIND_INT: u64 = 0b000;
    const KIND_TAG: u64 = 0b001;
    const KIND_TEXT: u64 = 0b010;
    const KIND_FUNCTION: u64 = 0b011;
    const KIND_LIST: u64 = 0b100;
    const KIND_STRUCT: u64 = 0b101;
    const KIND_HIR_ID: u64 = 0b110;

    pub const IS_REFERENCE_COUNTED_SHIFT: usize = 3;
    pub const IS_REFERENCE_COUNTED_MASK: u64 = 0b1 << Self::IS_REFERENCE_COUNTED_SHIFT;

    #[must_use]
    pub const fn new(address: NonNull<u64>) -> Self {
        Self(address)
    }

    #[must_use]
    pub const fn address(self) -> NonNull<u64> {
        self.0
    }
    #[must_use]
    pub fn pointer_equals(self, other: Self) -> bool {
        self.0 == other.0
    }
    #[must_use]
    pub fn unsafe_get_word(self, offset: usize) -> u64 {
        unsafe { *self.word_pointer(offset).as_ref() }
    }
    #[must_use]
    pub fn word_pointer(self, offset: usize) -> NonNull<u64> {
        self.0
            .map_addr(|it| it.checked_add(offset * Self::WORD_SIZE).unwrap())
    }
    #[must_use]
    pub fn header_word(self) -> u64 {
        self.unsafe_get_word(0)
    }

    // Reference Counting
    #[must_use]
    pub(super) fn is_reference_counted(self) -> bool {
        self.header_word() & Self::IS_REFERENCE_COUNTED_MASK != 0
    }
    fn reference_count_pointer(self) -> Option<NonNull<u64>> {
        if self.is_reference_counted() {
            Some(self.word_pointer(1))
        } else {
            None
        }
    }
    #[must_use]
    pub fn reference_count(&self) -> Option<usize> {
        self.reference_count_pointer().map(|it| {
            #[allow(clippy::cast_possible_truncation)]
            unsafe {
                *it.as_ref() as usize
            }
        })
    }
    pub(super) fn set_reference_count(self, value: usize) {
        let mut pointer = self.reference_count_pointer().unwrap();
        unsafe { *pointer.as_mut() = value as u64 }
    }

    pub fn dup(self) {
        self.dup_by(1);
    }
    pub fn dup_by(self, amount: usize) {
        let Some(reference_count) = self.reference_count() else {
            return;
        };

        let new_reference_count = reference_count + amount;
        self.set_reference_count(new_reference_count);
        trace!("RefCount of {self:p} increased to {new_reference_count}. Value: {self:?}");
    }
    pub fn drop(self, heap: &mut Heap) {
        let Some(reference_count) = self.reference_count() else {
            return;
        };

        let new_reference_count = reference_count - 1;
        trace!("RefCount of {self:p} reduced to {new_reference_count}. Value: {self:?}");
        self.set_reference_count(new_reference_count);

        if new_reference_count == 0 {
            self.free(heap);
        }
    }
    pub(super) fn free(self, heap: &mut Heap) {
        trace!("Freeing object at {self:p}.");
        assert_eq!(self.reference_count().unwrap_or_default(), 0);
        let data = HeapData::from(self);
        if DEBUG_ALLOCATIONS {
            // No need for pluralization because our heap objects are longer
            // than one byte.
            debug!("Freeing {} bytes: {data:?}.", data.total_size());
        }
        data.drop_children(heap);
        heap.deallocate(data);
    }

    // Cloning
    #[must_use]
    pub fn clone_to_heap(self, heap: &mut Heap) -> Self {
        self.clone_to_heap_with_mapping(heap, &mut FxHashMap::default())
    }
    #[must_use]
    pub fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<Self, Self>,
    ) -> Self {
        match address_map.entry(self) {
            hash_map::Entry::Occupied(entry) => {
                let object = entry.get();
                if let Some(reference_count) = object.reference_count() {
                    object.set_reference_count(reference_count + 1);
                }
                *object
            }
            hash_map::Entry::Vacant(entry) => {
                let data = HeapData::from(self);
                let new_object = heap.allocate_raw(self.header_word(), data.content_size());
                entry.insert(new_object);
                data.clone_content_to_heap_with_mapping(heap, new_object, address_map);
                new_object
            }
        }
    }

    // Content
    #[must_use]
    pub fn content_word_pointer(self, offset: usize) -> NonNull<u64> {
        let offset = if self.is_reference_counted() {
            offset + 2
        } else {
            offset + 1
        };
        self.word_pointer(offset)
    }
    #[must_use]
    pub fn unsafe_get_content_word(self, offset: usize) -> u64 {
        unsafe { *self.content_word_pointer(offset).as_ref() }
    }
    pub(self) fn unsafe_set_content_word(self, offset: usize, value: u64) {
        unsafe { *self.content_word_pointer(offset).as_mut() = value }
    }
}

impl DebugDisplay for HeapObject {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        DebugDisplay::fmt(&HeapData::from(*self), f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(HeapObject);

impl Pointer for HeapObject {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:p}", self.address())
    }
}

impl Eq for HeapObject {}
impl PartialEq for HeapObject {
    fn eq(&self, other: &Self) -> bool {
        self.pointer_equals(*other) || HeapData::from(*self) == HeapData::from(*other)
    }
}
impl Hash for HeapObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        HeapData::from(*self).hash(state);
    }
}
impl Ord for HeapObject {
    fn cmp(&self, other: &Self) -> Ordering {
        Data::from(*self).cmp(&Data::from(*other))
    }
}
impl PartialOrd for HeapObject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[enum_dispatch]
pub trait HeapObjectTrait: Copy + Into<HeapObject> {
    // Number of content bytes following the header and reference count words.
    #[must_use]
    fn content_size(self) -> usize;
    #[must_use]
    fn total_size(self) -> usize {
        2 * HeapObject::WORD_SIZE + self.content_size()
    }
    #[must_use]
    fn address_range(self) -> Range<NonZeroUsize> {
        let start = self.into().address().addr();
        start..start.checked_add(self.total_size()).unwrap()
    }

    fn clone_content_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        clone: HeapObject,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    );

    /// Calls [`Heap::drop`] for all referenced [`HeapObject`]s and drops
    /// allocated Rust objects owned by this object.
    ///
    /// This method is called by [free] prior to deallocating the object's
    /// memory.
    fn drop_children(self, heap: &mut Heap);

    // TODO: This is temporary. Once we store everything in the heap (including
    // stuff like big int values and HIR IDs), we can remove this.
    fn deallocate_external_stuff(self);
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
#[enum_dispatch(HeapObjectTrait)]
pub enum HeapData {
    Int(HeapInt),
    List(HeapList),
    Struct(HeapStruct),
    Text(HeapText),
    Tag(HeapTag),
    Function(HeapFunction),
    HirId(HeapHirId),
}

impl DebugDisplay for HeapData {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Self::Int(int) => DebugDisplay::fmt(int, f, is_debug),
            Self::List(list) => DebugDisplay::fmt(list, f, is_debug),
            Self::Struct(struct_) => DebugDisplay::fmt(struct_, f, is_debug),
            Self::Text(text) => DebugDisplay::fmt(text, f, is_debug),
            Self::Tag(tag) => DebugDisplay::fmt(tag, f, is_debug),
            Self::Function(function) => DebugDisplay::fmt(function, f, is_debug),
            Self::HirId(hir_id) => DebugDisplay::fmt(hir_id, f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(HeapData);

#[allow(clippy::fallible_impl_from)]
impl From<HeapObject> for HeapData {
    fn from(object: HeapObject) -> Self {
        let header_word = object.header_word();
        match header_word & HeapObject::KIND_MASK {
            HeapObject::KIND_INT => {
                assert_eq!(
                    header_word & !HeapObject::IS_REFERENCE_COUNTED_MASK,
                    HeapObject::KIND_INT,
                );
                Self::Int(HeapInt::new_unchecked(object))
            }
            HeapObject::KIND_LIST => Self::List(HeapList::new_unchecked(object)),
            HeapObject::KIND_STRUCT => Self::Struct(HeapStruct::new_unchecked(object)),
            HeapObject::KIND_TAG => Self::Tag(HeapTag::new_unchecked(object)),
            HeapObject::KIND_TEXT => Self::Text(HeapText::new_unchecked(object)),
            HeapObject::KIND_FUNCTION => Self::Function(HeapFunction::new_unchecked(object)),
            HeapObject::KIND_HIR_ID => {
                assert_eq!(
                    header_word & !HeapObject::IS_REFERENCE_COUNTED_MASK,
                    HeapObject::KIND_HIR_ID,
                );
                Self::HirId(HeapHirId::new_unchecked(object))
            }
            tag => panic!("Invalid tag: {tag:b}"),
        }
    }
}
impl Deref for HeapData {
    type Target = HeapObject;

    fn deref(&self) -> &Self::Target {
        match &self {
            Self::Int(int) => int,
            Self::List(list) => list,
            Self::Struct(struct_) => struct_,
            Self::Text(text) => text,
            Self::Tag(tag) => tag,
            Self::Function(function) => function,
            Self::HirId(hir_id) => hir_id,
        }
    }
}
impl From<HeapData> for HeapObject {
    fn from(value: HeapData) -> Self {
        *value
    }
}

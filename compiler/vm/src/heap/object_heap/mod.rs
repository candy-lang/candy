use self::{
    function::HeapFunction, hir_id::HeapHirId, int::HeapInt, list::HeapList, struct_::HeapStruct,
    tag::HeapTag, text::HeapText,
};
use super::{Data, Heap};
use crate::utils::{impl_debug_display_via_debugdisplay, DebugDisplay};
use enum_dispatch::enum_dispatch;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    collections::hash_map,
    fmt::{self, Formatter, Pointer},
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::Deref,
    ptr::NonNull,
};

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
pub struct HeapObject<'h> {
    address: NonNull<u64>,
    phantom: PhantomData<&'h ()>,
}
impl<'h> HeapObject<'h> {
    pub const WORD_SIZE: usize = 8;

    const KIND_MASK: u64 = 0b111;
    const KIND_INT: u64 = 0b000;
    const KIND_LIST: u64 = 0b001;
    const KIND_STRUCT: u64 = 0b101;
    const KIND_TAG: u64 = 0b010;
    const KIND_TEXT: u64 = 0b110;
    const KIND_FUNCTION: u64 = 0b011;
    const KIND_HIR_ID: u64 = 0b111;

    pub fn new(address: NonNull<u64>) -> Self {
        Self {
            address,
            phantom: PhantomData,
        }
    }

    pub fn address(self) -> NonNull<u64> {
        self.address
    }
    pub fn pointer_equals(self, other: HeapObject) -> bool {
        self.address == other.address
    }
    pub fn unsafe_get_word(self, offset: usize) -> u64 {
        unsafe { *self.word_pointer(offset).as_ref() }
    }
    pub fn word_pointer(self, offset: usize) -> NonNull<u64> {
        self.address
            .map_addr(|it| it.checked_add(offset * Self::WORD_SIZE).unwrap())
    }
    pub fn header_word(self) -> u64 {
        self.unsafe_get_word(0)
    }

    // Reference Counting
    fn reference_count_pointer(self) -> NonNull<u64> {
        self.word_pointer(1)
    }
    pub fn reference_count(&self) -> usize {
        unsafe { *self.reference_count_pointer().as_ref() as usize }
    }
    pub(super) fn set_reference_count(&self, value: usize) {
        unsafe { *self.reference_count_pointer().as_mut() = value as u64 }
    }

    pub fn dup(self) {
        self.dup_by(1);
    }
    pub fn dup_by(self, amount: usize) {
        let new_reference_count = self.reference_count() + amount;
        self.set_reference_count(new_reference_count);
        trace!("RefCount of {self:p} increased to {new_reference_count}. Value: {self:?}");
    }
    pub fn drop(self, heap: &mut Heap<'h>) {
        let new_reference_count = self.reference_count() - 1;
        trace!("RefCount of {self:p} reduced to {new_reference_count}. Value: {self:?}");
        self.set_reference_count(new_reference_count);

        if new_reference_count == 0 {
            self.free(heap);
        }
    }
    pub(super) fn free(self, heap: &mut Heap<'h>) {
        trace!("Freeing object at {self:p}.");
        assert_eq!(self.reference_count(), 0);
        let data = HeapData::from(self);
        data.drop_children(heap);
        heap.deallocate(data);
    }

    // Cloning
    pub fn clone_to_heap<'t>(self, heap: &mut Heap<'t>) -> HeapObject<'t> {
        self.clone_to_heap_with_mapping(heap, &mut FxHashMap::default())
    }
    pub fn clone_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    ) -> HeapObject<'t> {
        match address_map.entry(self) {
            hash_map::Entry::Occupied(entry) => {
                let object = entry.get();
                object.set_reference_count(object.reference_count() + 1);
                *object
            }
            hash_map::Entry::Vacant(entry) => {
                let data = HeapData::from(self);
                let new_object = heap.allocate(self.header_word(), data.content_size());
                entry.insert(new_object);
                data.clone_content_to_heap_with_mapping(heap, new_object, address_map);
                new_object
            }
        }
    }

    // Content
    pub fn unsafe_get_content_word(self, offset: usize) -> u64 {
        unsafe { *self.content_word_pointer(offset).as_ref() }
    }
    pub(self) fn unsafe_set_content_word(self, offset: usize, value: u64) {
        unsafe { *self.content_word_pointer(offset).as_mut() = value }
    }
    pub fn content_word_pointer(self, offset: usize) -> NonNull<u64> {
        self.word_pointer(2 + offset)
    }
}

impl DebugDisplay for HeapObject<'_> {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        DebugDisplay::fmt(&HeapData::from(*self), f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(HeapObject<'_>);
impl Pointer for HeapObject<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:p}", self.address())
    }
}

impl Eq for HeapObject<'_> {}
impl PartialEq for HeapObject<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.pointer_equals(*other) || HeapData::from(*self) == HeapData::from(*other)
    }
}
impl Hash for HeapObject<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        HeapData::from(*self).hash(state);
    }
}
impl Ord for HeapObject<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        Data::from(*self).cmp(&Data::from(*other))
    }
}
impl PartialOrd for HeapObject<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[enum_dispatch]
pub trait HeapObjectTrait<'h>: Into<HeapObject<'h>> {
    // Number of content bytes following the header and reference count words.
    fn content_size(self) -> usize;

    fn clone_content_to_heap_with_mapping<'t>(
        self,
        heap: &mut Heap<'t>,
        clone: HeapObject<'t>,
        address_map: &mut FxHashMap<HeapObject<'h>, HeapObject<'t>>,
    );

    /// Calls [Heap::drop] for all referenced [HeapObject]s and drops allocated
    /// Rust objects owned by this object.
    ///
    /// This method is called by [free] prior to deallocating the object's
    /// memory.
    fn drop_children(self, heap: &mut Heap<'h>);

    // TODO: This is temporary. Once we store everything in the heap (including
    // stuff like big int values and HIR IDs), we can remove this.
    fn deallocate_external_stuff(self);
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
#[enum_dispatch(HeapObjectTrait)]
pub enum HeapData<'h> {
    Int(HeapInt<'h>),
    List(HeapList<'h>),
    Struct(HeapStruct<'h>),
    Text(HeapText<'h>),
    Tag(HeapTag<'h>),
    Function(HeapFunction<'h>),
    HirId(HeapHirId<'h>),
}

impl DebugDisplay for HeapData<'_> {
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
impl_debug_display_via_debugdisplay!(HeapData<'_>);

impl<'h> From<HeapObject<'h>> for HeapData<'h> {
    fn from(object: HeapObject<'h>) -> Self {
        let header_word = object.header_word();
        match header_word & HeapObject::KIND_MASK {
            HeapObject::KIND_INT => {
                assert_eq!(header_word, HeapObject::KIND_INT);
                HeapData::Int(HeapInt::new_unchecked(object))
            }
            HeapObject::KIND_LIST => HeapData::List(HeapList::new_unchecked(object)),
            HeapObject::KIND_STRUCT => HeapData::Struct(HeapStruct::new_unchecked(object)),
            HeapObject::KIND_TAG => HeapData::Tag(HeapTag::new_unchecked(object)),
            HeapObject::KIND_TEXT => HeapData::Text(HeapText::new_unchecked(object)),
            HeapObject::KIND_FUNCTION => HeapData::Function(HeapFunction::new_unchecked(object)),
            HeapObject::KIND_HIR_ID => {
                assert_eq!(header_word, HeapObject::KIND_HIR_ID);
                HeapData::HirId(HeapHirId::new_unchecked(object))
            }
            tag => panic!("Invalid tag: {tag:b}"),
        }
    }
}
impl<'h> Deref for HeapData<'h> {
    type Target = HeapObject<'h>;

    fn deref(&self) -> &Self::Target {
        match &self {
            HeapData::Int(int) => int,
            HeapData::List(list) => list,
            HeapData::Struct(struct_) => struct_,
            HeapData::Text(text) => text,
            HeapData::Tag(tag) => tag,
            HeapData::Function(function) => function,
            HeapData::HirId(hir_id) => hir_id,
        }
    }
}
impl<'h> From<HeapData<'h>> for HeapObject<'h> {
    fn from(value: HeapData<'h>) -> Self {
        *value
    }
}

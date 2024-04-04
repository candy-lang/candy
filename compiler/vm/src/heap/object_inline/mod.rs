use self::{
    builtin::InlineBuiltin, handle::InlineHandle, int::InlineInt, pointer::InlinePointer,
    tag::InlineTag,
};
use super::{object_heap::HeapObject, Data, Heap};
use crate::{
    handle_id::HandleId,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::format::{format_value, FormatValue, MaxLength, Precedence};
use enum_dispatch::enum_dispatch;
use extension_trait::extension_trait;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
    ops::Deref,
};

pub(super) mod builtin;
pub(super) mod handle;
pub(super) mod int;
pub(super) mod pointer;
pub(super) mod tag;

#[extension_trait]
pub impl InlineObjectSliceCloneToHeap for [InlineObject] {
    fn clone_to_heap(&self, heap: &mut Heap) -> Vec<InlineObject> {
        self.clone_to_heap_with_mapping(heap, &mut FxHashMap::default())
    }
    fn clone_to_heap_with_mapping(
        &self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Vec<InlineObject> {
        self.iter()
            .map(|&item| item.clone_to_heap_with_mapping(heap, address_map))
            .collect()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct InlineObject(NonZeroU64);

impl InlineObject {
    pub const BITS: u32 = NonZeroU64::BITS;

    pub const KIND_WIDTH: usize = 3;
    pub const KIND_MASK: u64 = 0b111;
    pub const KIND_POINTER: u64 = 0b000;
    pub const KIND_INT: u64 = 0b001;
    pub const KIND_BUILTIN: u64 = 0b010;
    pub const KIND_TAG: u64 = 0b011;
    pub const KIND_HANDLE: u64 = 0b100;

    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn raw_word(self) -> NonZeroU64 {
        self.0
    }

    // Reference Counting
    pub fn dup(self, heap: &mut Heap) {
        self.dup_by(heap, 1);
    }
    pub fn dup_by(self, heap: &mut Heap, amount: usize) {
        if let Some(handle) = InlineData::from(self).handle_id() {
            heap.dup_handle_by(handle, amount);
        };

        match InlineData::from(self) {
            InlineData::Pointer(pointer) => pointer.get().dup_by(amount),
            InlineData::Tag(tag) => tag.dup_by(amount),
            _ => {}
        }
    }
    pub fn drop(self, heap: &mut Heap) {
        if let Some(handle) = InlineData::from(self).handle_id() {
            heap.drop_handle(handle);
        };

        match InlineData::from(self) {
            InlineData::Pointer(pointer) => pointer.get().drop(heap),
            InlineData::Tag(tag) => tag.drop(heap),
            _ => {}
        }
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
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        *InlineData::from(self).clone_to_heap_with_mapping(heap, address_map)
    }
}

impl DebugDisplay for InlineObject {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        InlineData::from(*self).fmt(f, is_debug)
    }
}
impl_debug_display_via_debugdisplay!(InlineObject);

impl Eq for InlineObject {}
impl PartialEq for InlineObject {
    fn eq(&self, other: &Self) -> bool {
        self.raw_word() == other.raw_word() || InlineData::from(*self) == InlineData::from(*other)
    }
}
impl Hash for InlineObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        InlineData::from(*self).hash(state);
    }
}
impl Ord for InlineObject {
    fn cmp(&self, other: &Self) -> Ordering {
        Data::from(*self).cmp(&Data::from(*other))
    }
}
impl PartialOrd for InlineObject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<InlineObject> for HeapObject {
    type Error = ();

    fn try_from(object: InlineObject) -> Result<Self, Self::Error> {
        match InlineData::from(object) {
            InlineData::Pointer(value) => Ok(value.get()),
            _ => Err(()),
        }
    }
}

#[enum_dispatch]
pub trait InlineObjectTrait: Copy + DebugDisplay + Eq + Hash {
    #[must_use]
    fn clone_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
#[enum_dispatch(InlineObjectTrait)]
pub enum InlineData {
    Pointer(InlinePointer),
    Int(InlineInt),
    Builtin(InlineBuiltin),
    Tag(InlineTag),
    Handle(InlineHandle),
}
impl InlineData {
    fn handle_id(&self) -> Option<HandleId> {
        match self {
            Self::Handle(handle) => Some(handle.handle_id()),
            _ => None,
        }
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<InlineObject> for InlineData {
    fn from(object: InlineObject) -> Self {
        let value = object.0.get();
        match value & InlineObject::KIND_MASK {
            InlineObject::KIND_POINTER => Self::Pointer(InlinePointer::new_unchecked(object)),
            InlineObject::KIND_INT => Self::Int(InlineInt::new_unchecked(object)),
            InlineObject::KIND_BUILTIN => Self::Builtin(InlineBuiltin::new_unchecked(object)),
            InlineObject::KIND_TAG => Self::Tag(InlineTag::new_unchecked(object)),
            InlineObject::KIND_HANDLE => Self::Handle(InlineHandle::new_unchecked(object)),
            _ => panic!("Unknown inline value type: {value:016x}"),
        }
    }
}

impl DebugDisplay for InlineData {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        match self {
            Self::Pointer(value) => value.fmt(f, is_debug),
            Self::Int(value) => value.fmt(f, is_debug),
            Self::Builtin(value) => value.fmt(f, is_debug),
            Self::Tag(value) => value.fmt(f, is_debug),
            Self::Handle(value) => value.fmt(f, is_debug),
        }
    }
}
impl_debug_display_via_debugdisplay!(InlineData);

impl Deref for InlineData {
    type Target = InlineObject;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Pointer(value) => value,
            Self::Int(value) => value,
            Self::Builtin(value) => value,
            Self::Tag(value) => value,
            Self::Handle(value) => value,
        }
    }
}

#[extension_trait]
pub impl ToDebugText for InlineObject {
    fn to_debug_text(self, precendence: Precedence, max_length: MaxLength) -> String {
        format_value(self, precendence, max_length, &|value| {
            Some(match value.into() {
                Data::Int(int) => FormatValue::Int(int.get()),
                Data::Tag(tag) => FormatValue::Tag {
                    symbol: tag.symbol().get(),
                    value: tag.value(),
                },
                Data::Text(text) => FormatValue::Text(text.get()),
                Data::List(list) => FormatValue::List(list.items()),
                Data::Struct(struct_) => FormatValue::Struct(Cow::Owned(
                    struct_
                        .iter()
                        .map(|(_, key, value)| (key, value))
                        .collect_vec(),
                )),
                Data::HirId(_) => unreachable!(),
                Data::Function(_) | Data::Builtin(_) | Data::Handle(_) => FormatValue::Function,
            })
        })
        .unwrap()
    }
}

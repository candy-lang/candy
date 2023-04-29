pub use self::{
    object::{
        Builtin, Closure, Data, DataDiscriminants, HirId, Int, List, ReceivePort, SendPort, Struct,
        Tag, Text,
    },
    object_heap::{HeapData, HeapObject, HeapObjectTrait},
    object_inline::{
        int::I64BitLength, InlineData, InlineObject, InlineObjectSliceCloneToHeap,
        InlineObjectTrait,
    },
    pointer::Pointer,
};
use crate::channel::ChannelId;
use derive_more::{DebugCustom, Deref, Pointer};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    alloc::{self, Allocator, Layout},
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    mem,
};

mod object;
mod object_heap;
mod object_inline;
mod pointer;

#[derive(Clone, Default)]
pub struct Heap {
    objects: FxHashSet<ObjectInHeap>,
    channel_refcounts: FxHashMap<ChannelId, usize>,
}

impl Heap {
    pub fn allocate(&mut self, header_word: u64, content_size: usize) -> HeapObject {
        let layout = Layout::from_size_align(
            2 * HeapObject::WORD_SIZE + content_size,
            HeapObject::WORD_SIZE,
        )
        .unwrap();

        // TODO: Handle allocation failure by stopping the fiber.
        let pointer = alloc::Global
            .allocate(layout)
            .expect("Not enough memory.")
            .cast();
        unsafe { *pointer.as_ptr() = header_word };
        let object = HeapObject::new(pointer);
        object.set_reference_count(1);
        self.objects.insert(ObjectInHeap(object));
        object
    }
    /// Don't call this method directly, call [drop] or [free] instead!
    pub(super) fn deallocate(&mut self, object: HeapData) {
        let layout = Layout::from_size_align(
            2 * HeapObject::WORD_SIZE + object.content_size(),
            HeapObject::WORD_SIZE,
        )
        .unwrap();
        self.objects.remove(&ObjectInHeap(*object));
        unsafe { alloc::Global.deallocate(object.address().cast(), layout) };
    }

    pub(self) fn notify_port_created(&mut self, channel_id: ChannelId) {
        self.channel_refcounts
            .entry(channel_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
    pub(self) fn dup_channel_by(&mut self, channel_id: ChannelId, amount: usize) {
        *self.channel_refcounts.entry(channel_id).or_insert_with(|| {
            panic!("Called `dup_channel_by`, but {channel_id:?} doesn't exist.")
        }) += amount;
    }
    pub(self) fn drop_channel(&mut self, channel_id: ChannelId) {
        let channel_refcount = self
            .channel_refcounts
            .entry(channel_id)
            .or_insert_with(|| panic!("Called `drop_channel`, but {channel_id:?} doesn't exist."));
        *channel_refcount -= 1;
        if *channel_refcount == 0 {
            self.channel_refcounts.remove(&channel_id).unwrap();
        }
    }

    pub fn objects_len(&self) -> usize {
        self.objects.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = HeapObject> + '_ {
        self.objects.iter().map(|it| **it)
    }

    pub fn known_channels(&self) -> impl IntoIterator<Item = ChannelId> + '_ {
        self.channel_refcounts.keys().copied()
    }

    pub fn clear(&mut self) {
        for object in mem::take(&mut self.objects).iter() {
            object.free(self);
        }
        self.channel_refcounts.clear();
    }
}

impl Debug for Heap {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{{")?;
        for &object in self.objects.iter() {
            let reference_count = object.reference_count();
            writeln!(
                f,
                "  {object:p} ({reference_count} {}): {object:?}",
                if reference_count == 1 { "ref" } else { "refs" },
            )?;
        }
        write!(f, "}}")
    }
}

/// For tracking objects allocated in the heap, we don't want deep equality, but
/// only care about the addresses.
#[derive(Clone, Copy, DebugCustom, Deref, Pointer)]
pub struct ObjectInHeap(pub HeapObject);

impl Eq for ObjectInHeap {}
impl PartialEq for ObjectInHeap {
    fn eq(&self, other: &Self) -> bool {
        self.0.address() == other.0.address()
    }
}

impl Hash for ObjectInHeap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.address().hash(state)
    }
}

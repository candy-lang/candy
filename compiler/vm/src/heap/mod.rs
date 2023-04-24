pub use self::{
    object::{
        Builtin, Closure, Data, HirId, Int, List, ReceivePort, SendPort, Struct, Symbol, Text,
    },
    object_heap::{HeapData, HeapObject, HeapObjectTrait},
    object_inline::{
        int::I64BitLength, InlineData, InlineObject, InlineObjectSliceCloneToHeap,
        InlineObjectTrait,
    },
    pointer::Pointer,
};
use crate::channel::ChannelId;
use rustc_hash::FxHashMap;
use std::{
    alloc::{self, Allocator, Layout},
    fmt::{self, Debug, Formatter},
};

mod object;
mod object_heap;
mod object_inline;
mod pointer;

#[derive(Clone, Default)]
pub struct Heap {
    objects: Vec<HeapObject>,
    channel_refcounts: FxHashMap<ChannelId, usize>,
}

impl Debug for Heap {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{{")?;
        for &object in &self.objects {
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
        let object = HeapObject(pointer);
        object.set_reference_count(1);
        self.objects.push(object);
        object
    }
    /// Don't call this method directly, call [drop] or [free] instead!
    pub(super) fn deallocate(&mut self, object: HeapData) {
        let layout = Layout::from_size_align(
            2 * HeapObject::WORD_SIZE + object.content_size(),
            HeapObject::WORD_SIZE,
        )
        .unwrap();
        unsafe { alloc::Global.deallocate(object.word_pointer(0).cast(), layout) };
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

    pub fn all_objects(&self) -> &[HeapObject] {
        &self.objects
    }

    pub fn known_channels(&self) -> impl IntoIterator<Item = ChannelId> + '_ {
        self.channel_refcounts.keys().copied()
    }
}

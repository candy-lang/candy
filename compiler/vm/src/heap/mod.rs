pub use self::{
    object::{
        Builtin, Data, DataDiscriminants, Function, Handle, HirId, Int, List, Struct, Tag, Text,
    },
    object_heap::{HeapData, HeapObject, HeapObjectTrait},
    object_inline::{
        int::I64BitLength, InlineData, InlineObject, InlineObjectSliceCloneToHeap,
        InlineObjectTrait, ToDebugText,
    },
    pointer::Pointer,
    symbol_table::{DisplayWithSymbolTable, OrdWithSymbolTable, SymbolId, SymbolTable},
};
use crate::handle_id::HandleId;
use candy_frontend::id::IdGenerator;
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
mod symbol_table;

#[derive(Default)]
pub struct Heap {
    objects: FxHashSet<ObjectInHeap>,
    handle_id_generator: IdGenerator<HandleId>,
    handle_refcounts: FxHashMap<HandleId, usize>,
}

impl Heap {
    pub fn allocate(
        &mut self,
        kind_bits: u64,
        is_reference_counted: bool,
        remaining_header_word: u64,
        content_size: usize,
    ) -> HeapObject {
        debug_assert_eq!(kind_bits & !HeapObject::KIND_MASK, 0);
        debug_assert_eq!(
            remaining_header_word & (HeapObject::KIND_MASK | HeapObject::IS_REFERENCE_COUNTED_MASK),
            0,
        );
        let header_word = kind_bits
            | (u64::from(is_reference_counted) << HeapObject::IS_REFERENCE_COUNTED_SHIFT)
            | remaining_header_word;
        self.allocate_raw(header_word, content_size)
    }
    pub fn allocate_raw(&mut self, header_word: u64, content_size: usize) -> HeapObject {
        let layout = Layout::from_size_align(
            2 * HeapObject::WORD_SIZE + content_size,
            HeapObject::WORD_SIZE,
        )
        .unwrap();

        // TODO: Handle allocation failure by stopping the VM.
        let pointer = alloc::Global
            .allocate(layout)
            .expect("Not enough memory.")
            .cast();
        unsafe { *pointer.as_ptr() = header_word };
        let object = HeapObject::new(pointer);
        if object.is_reference_counted() {
            object.set_reference_count(1);
        }
        self.objects.insert(ObjectInHeap(object));
        object
    }
    /// Don't call this method directly, call [drop] or [free] instead!
    pub(super) fn deallocate(&mut self, object: HeapData) {
        object.deallocate_external_stuff();
        let layout = Layout::from_size_align(
            2 * HeapObject::WORD_SIZE + object.content_size(),
            HeapObject::WORD_SIZE,
        )
        .unwrap();
        self.objects.remove(&ObjectInHeap(*object));
        unsafe { alloc::Global.deallocate(object.address().cast(), layout) };
    }

    pub(self) fn notify_handle_created(&mut self, handle_id: HandleId) {
        *self.handle_refcounts.entry(handle_id).or_default() += 1;
    }
    pub(self) fn dup_handle_by(&mut self, handle_id: HandleId, amount: usize) {
        *self.handle_refcounts.entry(handle_id).or_insert_with(|| {
            panic!("Called `dup_handle_by`, but {handle_id:?} doesn't exist.")
        }) += amount;
    }
    pub(self) fn drop_handle(&mut self, handle_id: HandleId) {
        let handle_refcount = self
            .handle_refcounts
            .entry(handle_id)
            .or_insert_with(|| panic!("Called `drop_handle`, but {handle_id:?} doesn't exist."));
        *handle_refcount -= 1;
        if *handle_refcount == 0 {
            self.handle_refcounts.remove(&handle_id).unwrap();
        }
    }

    pub fn adopt(&mut self, mut other: Self) {
        self.objects.extend(mem::take(&mut other.objects));
        for (handle_id, refcount) in mem::take(&mut other.handle_refcounts) {
            *self.handle_refcounts.entry(handle_id).or_default() += refcount;
        }
    }

    #[must_use]
    pub fn objects(&self) -> &FxHashSet<ObjectInHeap> {
        &self.objects
    }
    pub fn iter(&self) -> impl Iterator<Item = HeapObject> + '_ {
        self.objects.iter().map(|it| **it)
    }

    #[must_use]
    pub fn known_handles(&self) -> impl IntoIterator<Item = HandleId> + '_ {
        self.handle_refcounts.keys().copied()
    }

    // We do not confuse this with the `std::Clone::clone` method.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn clone(&self) -> (Self, FxHashMap<HeapObject, HeapObject>) {
        let mut cloned = Self {
            objects: FxHashSet::default(),
            handle_id_generator: self.handle_id_generator.clone(),
            handle_refcounts: self.handle_refcounts.clone(),
        };

        let mut mapping = FxHashMap::default();
        for object in &self.objects {
            _ = object.clone_to_heap_with_mapping(&mut cloned, &mut mapping);
        }

        (cloned, mapping)
    }

    pub fn clear(&mut self) {
        for object in mem::take(&mut self.objects) {
            self.deallocate(HeapData::from(object.0));
        }
        self.handle_refcounts.clear();
    }
}

impl Debug for Heap {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{{\n  handle_refcounts: {:?}", self.handle_refcounts)?;

        for &object in &self.objects {
            writeln!(
                f,
                "  {object:p}{}: {object:?}",
                if let Some(reference_count) = object.reference_count() {
                    format!(
                        " ({reference_count} {})",
                        if reference_count == 1 { "ref" } else { "refs" },
                    )
                } else {
                    String::new()
                },
            )?;
        }
        write!(f, "}}")
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        self.clear();
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
        self.0.address().hash(state);
    }
}

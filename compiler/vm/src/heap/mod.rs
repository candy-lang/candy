use self::object_heap::text::HeapText;
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

pub struct Heap {
    objects: FxHashSet<ObjectInHeap>,
    default_symbols: Option<DefaultSymbols>,
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
    pub const fn objects(&self) -> &FxHashSet<ObjectInHeap> {
        &self.objects
    }
    pub fn iter(&self) -> impl Iterator<Item = HeapObject> + '_ {
        self.objects.iter().map(|it| **it)
    }

    #[must_use]
    pub fn default_symbols(&self) -> &DefaultSymbols {
        self.default_symbols.as_ref().unwrap()
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
            default_symbols: None,
            handle_id_generator: self.handle_id_generator.clone(),
            handle_refcounts: self.handle_refcounts.clone(),
        };

        let mut mapping = FxHashMap::default();
        cloned.default_symbols = Some(
            self.default_symbols
                .as_ref()
                .unwrap()
                .clone_to_heap_with_mapping(&mut cloned, &mut mapping),
        );

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
                object
                    .reference_count()
                    .map_or_else(String::new, |reference_count| format!(
                        " ({reference_count} {})",
                        if reference_count == 1 { "ref" } else { "refs" },
                    )),
            )?;
        }
        write!(f, "}}")
    }
}

impl Default for Heap {
    fn default() -> Self {
        let mut heap = Self {
            objects: FxHashSet::default(),
            default_symbols: None,
            handle_id_generator: IdGenerator::default(),
            handle_refcounts: FxHashMap::default(),
        };
        heap.default_symbols = Some(DefaultSymbols::new(&mut heap));
        heap
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

pub struct DefaultSymbols {
    // These symbols are created by built-in functions or used for starting the
    // program (main and environment keys). They are created once so that they
    // can be used in the VM without new allocations.
    //
    // When adding a new default symbol, you have to update `new(…)`,
    // `clone_to_heap_with_mapping(…)`, and `all_symbols(…)`.
    //
    // Sorted alphabetically
    pub arguments: Text,
    pub builtin: Text,
    pub equal: Text,
    pub error: Text,
    pub false_: Text,
    pub function: Text,
    pub get_random_bytes: Text,
    pub greater: Text,
    pub int: Text,
    pub less: Text,
    pub list: Text,
    pub main: Text,
    pub not_an_integer: Text,
    pub not_utf8: Text,
    pub nothing: Text,
    pub ok: Text,
    pub stdin: Text,
    pub stdout: Text,
    pub struct_: Text,
    pub tag: Text,
    pub text: Text,
    pub true_: Text,
}
impl DefaultSymbols {
    pub fn new(heap: &mut Heap) -> Self {
        Self {
            arguments: Text::create(heap, false, "Arguments"),
            builtin: Text::create(heap, false, "Builtin"),
            equal: Text::create(heap, false, "Equal"),
            error: Text::create(heap, false, "Error"),
            false_: Text::create(heap, false, "False"),
            function: Text::create(heap, false, "Function"),
            get_random_bytes: Text::create(heap, false, "GetRandomBytes"),
            greater: Text::create(heap, false, "Greater"),
            int: Text::create(heap, false, "Int"),
            less: Text::create(heap, false, "Less"),
            list: Text::create(heap, false, "List"),
            main: Text::create(heap, false, "Main"),
            not_an_integer: Text::create(heap, false, "NotAnInteger"),
            not_utf8: Text::create(heap, false, "NotUtf8"),
            nothing: Text::create(heap, false, "Nothing"),
            ok: Text::create(heap, false, "Ok"),
            stdin: Text::create(heap, false, "Stdin"),
            stdout: Text::create(heap, false, "Stdout"),
            struct_: Text::create(heap, false, "Struct"),
            tag: Text::create(heap, false, "Tag"),
            text: Text::create(heap, false, "Text"),
            true_: Text::create(heap, false, "True"),
        }
    }
    fn clone_to_heap_with_mapping(
        &self,
        heap: &mut Heap,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        fn clone_to_heap(
            heap: &mut Heap,
            address_map: &mut FxHashMap<HeapObject, HeapObject>,
            text: Text,
        ) -> Text {
            let cloned = text.clone_to_heap_with_mapping(heap, address_map);
            HeapText::new_unchecked(cloned).into()
        }

        Self {
            arguments: clone_to_heap(heap, address_map, self.arguments),
            builtin: clone_to_heap(heap, address_map, self.builtin),
            equal: clone_to_heap(heap, address_map, self.equal),
            error: clone_to_heap(heap, address_map, self.error),
            false_: clone_to_heap(heap, address_map, self.false_),
            function: clone_to_heap(heap, address_map, self.function),
            get_random_bytes: clone_to_heap(heap, address_map, self.get_random_bytes),
            greater: clone_to_heap(heap, address_map, self.greater),
            int: clone_to_heap(heap, address_map, self.int),
            less: clone_to_heap(heap, address_map, self.less),
            list: clone_to_heap(heap, address_map, self.list),
            main: clone_to_heap(heap, address_map, self.main),
            not_an_integer: clone_to_heap(heap, address_map, self.not_an_integer),
            not_utf8: clone_to_heap(heap, address_map, self.not_utf8),
            nothing: clone_to_heap(heap, address_map, self.nothing),
            ok: clone_to_heap(heap, address_map, self.ok),
            stdin: clone_to_heap(heap, address_map, self.stdin),
            stdout: clone_to_heap(heap, address_map, self.stdout),
            struct_: clone_to_heap(heap, address_map, self.struct_),
            tag: clone_to_heap(heap, address_map, self.tag),
            text: clone_to_heap(heap, address_map, self.text),
            true_: clone_to_heap(heap, address_map, self.true_),
        }
    }

    #[must_use]
    pub fn get(&self, text: &str) -> Option<Text> {
        let symbols = self.all_symbols();
        symbols
            .binary_search_by_key(&text, |it| it.get())
            .ok()
            .map(|it| symbols[it])
    }
    #[must_use]
    pub const fn all_symbols(&self) -> [Text; 22] {
        [
            self.arguments,
            self.builtin,
            self.equal,
            self.error,
            self.false_,
            self.function,
            self.get_random_bytes,
            self.greater,
            self.int,
            self.less,
            self.list,
            self.main,
            self.not_an_integer,
            self.not_utf8,
            self.nothing,
            self.ok,
            self.stdin,
            self.stdout,
            self.struct_,
            self.tag,
            self.text,
            self.true_,
        ]
    }
}

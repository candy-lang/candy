use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    instruction_pointer::InstructionPointer,
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use derive_more::Deref;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    ptr::{self, NonNull},
    slice,
};

#[derive(Clone, Copy, Deref)]
pub struct HeapFunction(HeapObject);

impl HeapFunction {
    const CAPTURED_LEN_SHIFT: usize = 32;
    const ARGUMENT_COUNT_SHIFT: usize = 4;

    #[must_use]
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    #[must_use]
    pub fn create(
        heap: &mut Heap,
        is_reference_counted: bool,
        captured: &[InlineObject],
        argument_count: usize,
        body: InstructionPointer,
    ) -> Self {
        let captured_len = captured.len();
        assert_eq!(
            (captured_len << Self::CAPTURED_LEN_SHIFT) >> Self::CAPTURED_LEN_SHIFT,
            captured_len,
            "Function captures too many things.",
        );

        let argument_count_shift_for_max_size =
            InlineObject::BITS as usize - Self::CAPTURED_LEN_SHIFT + Self::ARGUMENT_COUNT_SHIFT;
        assert_eq!(
            (argument_count << argument_count_shift_for_max_size)
                >> argument_count_shift_for_max_size,
            argument_count,
            "Function accepts too many arguments.",
        );

        let function = Self(heap.allocate(
            HeapObject::KIND_FUNCTION,
            is_reference_counted,
            ((captured_len as u64) << Self::CAPTURED_LEN_SHIFT)
                | ((argument_count as u64) << Self::ARGUMENT_COUNT_SHIFT),
            (1 + captured_len) * HeapObject::WORD_SIZE,
        ));
        unsafe {
            *function.body_pointer().as_mut() = *body as u64;
            ptr::copy_nonoverlapping(
                captured.as_ptr(),
                function.captured_pointer().as_ptr(),
                captured_len,
            );
        }

        function
    }

    #[must_use]
    pub fn captured_len(self) -> usize {
        (self.header_word() >> Self::CAPTURED_LEN_SHIFT) as usize
    }
    #[must_use]
    fn captured_pointer(self) -> NonNull<InlineObject> {
        self.content_word_pointer(1).cast()
    }
    #[must_use]
    pub fn captured<'a>(self) -> &'a [InlineObject] {
        unsafe { slice::from_raw_parts(self.captured_pointer().as_ptr(), self.captured_len()) }
    }

    #[must_use]
    pub fn argument_count(self) -> usize {
        ((self.header_word() & 0xFFFF_FFFF) >> Self::ARGUMENT_COUNT_SHIFT) as usize
    }

    #[must_use]
    fn body_pointer(self) -> NonNull<u64> {
        self.content_word_pointer(0)
    }
    #[must_use]
    pub fn body(self) -> InstructionPointer {
        #[allow(clippy::cast_possible_truncation)]
        unsafe { *self.body_pointer().as_ref() as usize }.into()
    }
}

impl DebugDisplay for HeapFunction {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        if is_debug {
            let argument_count = self.argument_count();
            let captured = self.captured();
            write!(
                f,
                "{{ {} {} (capturing {}) → {:?} }}",
                argument_count,
                if argument_count == 1 {
                    "argument"
                } else {
                    "arguments"
                },
                if captured.is_empty() {
                    "nothing".to_string()
                } else {
                    captured.iter().map(|it| format!("{it:?}")).join(", ")
                },
                self.body(),
            )
        } else {
            write!(f, "{{…}}")
        }
    }
}
impl_debug_display_via_debugdisplay!(HeapFunction);

impl Eq for HeapFunction {}
impl PartialEq for HeapFunction {
    fn eq(&self, other: &Self) -> bool {
        // TODO: Compare the underlying HIR ID once we have it here (plus captured stuff)
        self.captured() == other.captured()
            && self.argument_count() == other.argument_count()
            && self.body() == other.body()
    }
}

impl Hash for HeapFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.captured().hash(state);
        self.argument_count().hash(state);
        self.body().hash(state);
    }
}

impl Ord for HeapFunction {
    fn cmp(&self, other: &Self) -> Ordering {
        // TODO: Compare the underlying HIR ID once we have it here (plus captured stuff)
        self.address().cmp(&other.address())
    }
}
impl PartialOrd for HeapFunction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

heap_object_impls!(HeapFunction);

impl HeapObjectTrait for HeapFunction {
    fn content_size(self) -> usize {
        (1 + self.captured_len()) * HeapObject::WORD_SIZE
    }

    fn clone_content_to_heap_with_mapping(
        self,
        heap: &mut Heap,
        clone: HeapObject,
        address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        unsafe { *clone.body_pointer().as_mut() = *self.body() as u64 };
        for (index, &captured) in self.captured().iter().enumerate() {
            clone.unsafe_set_content_word(
                1 + index,
                captured
                    .clone_to_heap_with_mapping(heap, address_map)
                    .raw_word()
                    .get(),
            );
        }
    }

    fn drop_children(self, heap: &mut Heap) {
        for captured in self.captured() {
            captured.drop(heap);
        }
    }

    fn deallocate_external_stuff(self) {}
}

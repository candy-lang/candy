use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{
        object_heap::{utils::RefCountToString, HeapObject},
        Heap, Int, List, Tag, Text,
    },
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get, DebugDisplay},
};
use derive_more::Deref;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    ops::Range,
    ptr::{self, NonNull},
    slice, str,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Deref)]
pub struct HeapText(HeapObject);

impl HeapText {
    const BYTE_LEN_SHIFT: usize = 4;

    #[must_use]
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    #[must_use]
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: &str) -> Self {
        let byte_len = value.len();
        assert_eq!(
            (byte_len << Self::BYTE_LEN_SHIFT) >> Self::BYTE_LEN_SHIFT,
            byte_len,
            "Text is too long.",
        );
        let text = Self(heap.allocate(
            HeapObject::KIND_TEXT,
            is_reference_counted,
            (byte_len as u64) << Self::BYTE_LEN_SHIFT,
            byte_len,
        ));
        unsafe { ptr::copy_nonoverlapping(value.as_ptr(), text.text_pointer().as_ptr(), byte_len) };
        text
    }

    #[must_use]
    pub fn byte_len(self) -> usize {
        (self.header_word() >> Self::BYTE_LEN_SHIFT) as usize
    }
    #[must_use]
    fn text_pointer(self) -> NonNull<u8> {
        self.content_word_pointer(0).cast()
    }
    #[must_use]
    pub fn get<'a>(self) -> &'a str {
        let pointer = self.text_pointer().as_ptr();
        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(pointer, self.byte_len())) }
    }

    #[must_use]
    pub fn is_empty(self, heap: &Heap) -> Tag {
        Tag::create_bool(heap, self.get().is_empty())
    }
    #[must_use]
    pub fn length(self, heap: &mut Heap) -> Int {
        Int::create(heap, true, self.get().graphemes(true).count())
    }
    #[must_use]
    pub fn characters(self, heap: &mut Heap) -> List {
        let characters = self
            .get()
            .graphemes(true)
            .map(|it| Text::create(heap, true, it).into())
            .collect_vec();
        List::create(heap, true, &characters)
    }
    #[must_use]
    pub fn contains(self, heap: &Heap, pattern: Text) -> Tag {
        Tag::create_bool(heap, self.get().contains(pattern.get()))
    }
    #[must_use]
    pub fn starts_with(self, heap: &Heap, prefix: Text) -> Tag {
        Tag::create_bool(heap, self.get().starts_with(prefix.get()))
    }
    #[must_use]
    pub fn ends_with(self, heap: &Heap, suffix: Text) -> Tag {
        Tag::create_bool(heap, self.get().ends_with(suffix.get()))
    }
    #[must_use]
    pub fn get_range(self, heap: &mut Heap, range: Range<Int>) -> Text {
        // TODO: Support indices larger than usize.
        let start_inclusive = range
            .start
            .try_get()
            .expect("Tried to get a range from a text with an index that's too large for usize.");
        let end_exclusive = range
            .end
            .try_get::<usize>()
            .expect("Tried to get a range from a text with an index that's too large for usize.");
        let text: String = self
            .get()
            .graphemes(true)
            .skip(start_inclusive)
            .take(end_exclusive - start_inclusive)
            .collect();
        Text::create(heap, true, &text)
    }

    #[must_use]
    pub fn concatenate(self, heap: &mut Heap, other: Text) -> Text {
        Text::create(heap, true, &format!("{}{}", self.get(), other.get()))
    }
    #[must_use]
    pub fn trim_start(self, heap: &mut Heap) -> Text {
        Text::create(heap, true, self.get().trim_start())
    }
    #[must_use]
    pub fn trim_end(self, heap: &mut Heap) -> Text {
        Text::create(heap, true, self.get().trim_end())
    }
}

impl DebugDisplay for HeapText {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        write!(f, "\"{}\"", self.get())?;
        if is_debug {
            write!(f, " [{}]", self.reference_count().ref_count_to_string())?;
        }
        Ok(())
    }
}
impl_debug_display_via_debugdisplay!(HeapText);

impl_eq_hash_ord_via_get!(HeapText);

heap_object_impls!(HeapText);

impl HeapObjectTrait for HeapText {
    fn content_size(self) -> usize {
        self.byte_len()
    }

    fn clone_content_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        clone: HeapObject,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        unsafe {
            ptr::copy_nonoverlapping(
                self.text_pointer().as_ptr(),
                clone.text_pointer().as_ptr(),
                self.byte_len(),
            );
        };
    }

    fn drop_children(self, _heap: &mut Heap) {}

    fn deallocate_external_stuff(self) {}
}

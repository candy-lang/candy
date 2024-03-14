use super::InlineObjectTrait;
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
    InstructionPointer,
};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

#[derive(Clone, Copy, Deref)]
pub struct InlineFunction(InlineObject);

impl InlineFunction {
    const BODY_SHIFT: usize = 8;
    const ARGUMENT_COUNT_SHIFT: usize = 3;

    #[must_use]
    pub const fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    #[must_use]
    pub fn try_create(argument_count: usize, body: InstructionPointer) -> Option<Self> {
        let argument_count_shift_for_max_size =
            InlineObject::BITS as usize - Self::BODY_SHIFT + Self::ARGUMENT_COUNT_SHIFT;
        if (argument_count << argument_count_shift_for_max_size)
            >> argument_count_shift_for_max_size
            != argument_count
        {
            return None;
        }

        let body = *body;
        if (body << Self::BODY_SHIFT) >> Self::BODY_SHIFT != body {
            return None;
        }

        let header_word = InlineObject::KIND_FUNCTION
            | ((body as u64) << Self::BODY_SHIFT)
            | ((argument_count as u64) << Self::ARGUMENT_COUNT_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Some(Self(InlineObject::new(header_word)))
    }

    #[must_use]
    pub fn argument_count(self) -> usize {
        ((self.raw_word().get() & 0xFF) >> Self::ARGUMENT_COUNT_SHIFT) as usize
    }

    #[must_use]
    pub fn body(self) -> InstructionPointer {
        let value = (self.raw_word().get() >> Self::BODY_SHIFT) as usize;
        InstructionPointer::from(value)
    }
}

impl DebugDisplay for InlineFunction {
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result {
        if is_debug {
            let argument_count = self.argument_count();
            write!(
                f,
                "{{ {} {} (capturing nothing) → {:?} }}",
                argument_count,
                if argument_count == 1 {
                    "argument"
                } else {
                    "arguments"
                },
                self.body(),
            )
        } else {
            write!(f, "{{…}}")
        }
    }
}
impl_debug_display_via_debugdisplay!(InlineFunction);

impl Eq for InlineFunction {}
impl PartialEq for InlineFunction {
    fn eq(&self, other: &Self) -> bool {
        self.body() == other.body()
    }
}
impl Hash for InlineFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.body().hash(state);
    }
}
impl Ord for InlineFunction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.body().cmp(&other.body())
    }
}
impl PartialOrd for InlineFunction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<InstructionPointer> for InlineObject {
    type Error = ();

    fn try_from(body: InstructionPointer) -> Result<Self, Self::Error> {
        InlineFunction::try_from(body).map(|it| *it)
    }
}
impl TryFrom<InstructionPointer> for InlineFunction {
    type Error = ();

    fn try_from(body: InstructionPointer) -> Result<Self, Self::Error> {
        if (*body << Self::BODY_SHIFT) >> Self::BODY_SHIFT != *body {
            return Err(());
        }

        let header_word = InlineObject::KIND_FUNCTION | ((*body as u64) << Self::BODY_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Ok(Self(InlineObject::new(header_word)))
    }
}

impl InlineObjectTrait for InlineFunction {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}

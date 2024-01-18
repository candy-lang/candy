use super::InlineObjectTrait;
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::builtin_functions::{self, BuiltinFunction};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroU64,
};

#[derive(Clone, Copy, Deref)]
pub struct InlineBuiltin(InlineObject);

impl InlineBuiltin {
    const INDEX_SHIFT: usize = 3;

    #[must_use]
    pub const fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    #[must_use]
    fn index(self) -> usize {
        (self.raw_word().get() >> Self::INDEX_SHIFT) as usize
    }
    #[must_use]
    pub fn get(self) -> BuiltinFunction {
        builtin_functions::VALUES[self.index()]
    }
}

impl DebugDisplay for InlineBuiltin {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(InlineBuiltin);

impl Eq for InlineBuiltin {}
impl PartialEq for InlineBuiltin {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
    }
}
impl Hash for InlineBuiltin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}
impl Ord for InlineBuiltin {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index().cmp(&other.index())
    }
}
impl PartialOrd for InlineBuiltin {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<BuiltinFunction> for InlineObject {
    fn from(builtin_function: BuiltinFunction) -> Self {
        *InlineBuiltin::from(builtin_function)
    }
}
impl From<BuiltinFunction> for InlineBuiltin {
    fn from(builtin_function: BuiltinFunction) -> Self {
        let index = builtin_function as usize;
        debug_assert_eq!(
            (index << Self::INDEX_SHIFT) >> Self::INDEX_SHIFT,
            index,
            "Builtin function index is too large.",
        );
        let header_word = InlineObject::KIND_BUILTIN | ((index as u64) << Self::INDEX_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Self(InlineObject::new(header_word))
    }
}

impl InlineObjectTrait for InlineBuiltin {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}

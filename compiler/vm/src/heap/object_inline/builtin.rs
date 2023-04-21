use super::InlineObjectTrait;
use crate::{
    heap::{object_heap::HeapObject, Heap, InlineObject},
    utils::{impl_debug_display_via_debugdisplay, DebugDisplay},
};
use candy_frontend::builtin_functions::{self, BuiltinFunction};
use derive_more::Deref;
use rustc_hash::FxHashMap;
use std::fmt::{self, Formatter};

#[derive(Clone, Copy, Deref, Eq, Hash, PartialEq)]
pub struct InlineBuiltin(InlineObject);

impl InlineBuiltin {
    const BUILTIN_FUNCTION_INDEX_SHIFT: usize = 2;

    pub fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    pub fn get(self) -> BuiltinFunction {
        let index = self.raw_word() >> Self::BUILTIN_FUNCTION_INDEX_SHIFT;
        builtin_functions::VALUES[index as usize]
    }
}

impl DebugDisplay for InlineBuiltin {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "builtin{:?}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(InlineBuiltin);

impl From<BuiltinFunction> for InlineObject {
    fn from(builtin_function: BuiltinFunction) -> Self {
        *InlineBuiltin::from(builtin_function)
    }
}
impl From<BuiltinFunction> for InlineBuiltin {
    fn from(builtin_function: BuiltinFunction) -> Self {
        let index = builtin_function as usize;
        debug_assert_eq!(
            (index << Self::BUILTIN_FUNCTION_INDEX_SHIFT) >> Self::BUILTIN_FUNCTION_INDEX_SHIFT,
            index,
            "Builtin function index is too large.",
        );
        let header_word =
            InlineObject::KIND_BUILTIN | ((index as u64) << Self::BUILTIN_FUNCTION_INDEX_SHIFT);
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

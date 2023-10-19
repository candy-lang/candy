use super::{utils::heap_object_impls, HeapObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, object_inline::int::InlineInt, Heap, Int, Tag},
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get, DebugDisplay},
};
use derive_more::Deref;
use num_bigint::BigInt;
use num_integer::Integer;
use rustc_hash::FxHashMap;
use std::{
    fmt::{self, Formatter},
    mem,
    ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub},
    ptr::{self, NonNull},
};

#[derive(Clone, Copy, Deref)]
pub struct HeapInt(HeapObject);

impl HeapInt {
    pub const fn new_unchecked(object: HeapObject) -> Self {
        Self(object)
    }
    pub fn create(heap: &mut Heap, is_reference_counted: bool, value: BigInt) -> Self {
        if let Ok(value) = i64::try_from(&value) {
            debug_assert!(!InlineInt::fits(value));
        }

        let int = Self(heap.allocate(
            HeapObject::KIND_INT,
            is_reference_counted,
            0,
            mem::size_of::<BigInt>(),
        ));
        unsafe { ptr::write(int.int_pointer().as_ptr(), value) };
        int
    }

    fn int_pointer(self) -> NonNull<BigInt> {
        self.content_word_pointer(0).cast()
    }
    pub fn get<'a>(self) -> &'a BigInt {
        unsafe { self.int_pointer().as_ref() }
    }

    operator_fn!(add, Add, add);
    operator_fn!(subtract, Sub, sub);
    operator_fn!(multiply, Mul, mul);
    operator_fn!(int_divide_truncating, Div, div);
    operator_fn!(remainder, Rem, rem);
    pub fn modulo(self, heap: &mut Heap, rhs: &BigInt) -> Int {
        Int::create_from_bigint(heap, true, self.get().mod_floor(rhs))
    }

    pub fn compare_to(self, heap: &Heap, rhs: &BigInt) -> Tag {
        // PERF: Add manual check if the `rhs` is an [InlineInt]?
        Tag::create_ordering(heap, self.get().cmp(rhs))
    }

    operator_fn!(shift_left, Shl, shl);
    operator_fn!(shift_right, Shr, shr);

    pub fn bit_length(self, heap: &mut Heap) -> Int {
        Int::create(heap, true, self.get().bits())
    }

    operator_fn!(bitwise_and, BitAnd, bitand);
    operator_fn!(bitwise_or, BitOr, bitor);
    operator_fn!(bitwise_xor, BitXor, bitxor);
}

macro_rules! operator_fn {
    ($name:ident, $trait:ident, $function:ident) => {
        pub fn $name<T>(self, heap: &mut Heap, rhs: T) -> Int
        where
            for<'a> &'a BigInt: $trait<T, Output = BigInt>,
        {
            let result = $trait::$function(self.get(), rhs);
            Int::create_from_bigint(heap, true, result)
        }
    };
}
use operator_fn;

impl DebugDisplay for HeapInt {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(HeapInt);

impl_eq_hash_ord_via_get!(HeapInt);

heap_object_impls!(HeapInt);

impl HeapObjectTrait for HeapInt {
    fn content_size(self) -> usize {
        mem::size_of::<BigInt>()
    }

    fn clone_content_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        clone: HeapObject,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) {
        let clone = Self(clone);
        let value = self.get().clone();
        unsafe { ptr::write(clone.int_pointer().as_ptr(), value) };
    }

    fn drop_children(self, _heap: &mut Heap) {}

    fn deallocate_external_stuff(self) {
        unsafe { ptr::drop_in_place(self.int_pointer().as_ptr()) };
    }
}

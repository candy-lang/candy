use super::{InlineObject, InlineObjectTrait};
use crate::{
    heap::{
        object_heap::HeapObject, symbol_table::impl_ord_with_symbol_table_via_ord, Heap, Int, Tag,
    },
    utils::{impl_debug_display_via_debugdisplay, impl_eq_hash_ord_via_get, DebugDisplay},
};
use derive_more::Deref;
use extension_trait::extension_trait;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Signed;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    fmt::{self, Formatter},
    num::NonZeroU64,
    ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub},
};

#[derive(Clone, Copy, Deref)]
pub struct InlineInt(InlineObject);
impl InlineInt {
    const VALUE_SHIFT: usize = 3;

    pub const fn new_unchecked(object: InlineObject) -> Self {
        Self(object)
    }

    pub const fn fits(value: i64) -> bool {
        (value << Self::VALUE_SHIFT) >> Self::VALUE_SHIFT == value
    }
    pub fn from_unchecked(value: i64) -> Self {
        debug_assert_eq!(
            (value << Self::VALUE_SHIFT) >> Self::VALUE_SHIFT,
            value,
            "Integer is too large.",
        );
        #[allow(clippy::cast_sign_loss)]
        let header_word = InlineObject::KIND_INT | ((value as u64) << Self::VALUE_SHIFT);
        let header_word = unsafe { NonZeroU64::new_unchecked(header_word) };
        Self(InlineObject(header_word))
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn get(self) -> i64 {
        self.raw_word().get() as i64 >> Self::VALUE_SHIFT
    }
    pub fn try_get<T: TryFrom<i64>>(self) -> Option<T> {
        self.get().try_into().ok()
    }

    operator_fn!(add, i64::checked_add, Add::add);
    operator_fn!(subtract, i64::checked_sub, Sub::sub);
    operator_fn!(multiply, i64::checked_mul, Mul::mul);
    operator_fn!(int_divide_truncating, i64::checked_div, Div::div);
    operator_fn!(remainder, i64::checked_rem, Rem::rem);
    pub fn modulo(self, heap: &mut Heap, rhs: Self) -> Int {
        let lhs = self.get();
        let rhs = rhs.get();
        #[allow(clippy::map_unwrap_or)]
        lhs.checked_rem_euclid(rhs)
            .map(|it| Int::create(heap, true, it))
            .unwrap_or_else(|| {
                Int::create_from_bigint(heap, true, BigInt::from(lhs).mod_floor(&rhs.into()))
            })
    }

    pub fn compare_to(self, rhs: Int) -> Tag {
        let ordering = match rhs {
            Int::Inline(rhs) => self.get().cmp(&rhs.get()),
            Int::Heap(rhs) => {
                if rhs.get().is_negative() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
        };
        Tag::create_ordering(ordering)
    }

    shift_fn!(shift_left, i64::checked_shl, Shl::shl);
    shift_fn!(shift_right, i64::checked_shr, Shr::shr);

    pub fn bit_length(self) -> Self {
        // SAFETY: The `bit_length` can be at most 61 since that's how large an [InlineInt] can get.
        Self::from_unchecked(self.get().bit_length().into())
    }

    operator_fn_closed!(bitwise_and, BitAnd::bitand);
    operator_fn_closed!(bitwise_or, BitOr::bitor);
    operator_fn_closed!(bitwise_xor, BitXor::bitxor);
}

macro_rules! operator_fn {
    ($name:ident, $inline_operation:expr, $bigint_operation:expr) => {
        pub fn $name(self, heap: &mut Heap, rhs: Int) -> Int {
            let lhs = self.get();
            match rhs {
                Int::Inline(rhs) => rhs
                    .try_get()
                    .and_then(|rhs| $inline_operation(lhs, rhs))
                    .map(|it| Int::create(heap, true, it))
                    .unwrap_or_else(|| {
                        Int::create_from_bigint(
                            heap,
                            true,
                            $bigint_operation(BigInt::from(lhs), rhs.get()),
                        )
                    }),
                Int::Heap(rhs) => Int::create_from_bigint(
                    heap,
                    true,
                    $bigint_operation(BigInt::from(lhs), rhs.get()),
                ),
            }
        }
    };
}
macro_rules! shift_fn {
    ($name:ident, $inline_operation:expr, $bigint_operation:expr) => {
        pub fn $name(self, heap: &mut Heap, rhs: InlineInt) -> Int {
            let lhs = self.get();
            rhs.try_get()
                .and_then(|rhs| $inline_operation(lhs, rhs))
                .map(|it| Int::create(heap, true, it))
                .unwrap_or_else(|| {
                    Int::create_from_bigint(
                        heap,
                        true,
                        $bigint_operation(BigInt::from(lhs), rhs.get()),
                    )
                })
        }
    };
}
macro_rules! operator_fn_closed {
    ($name:ident, $operation:expr) => {
        pub fn $name(self, rhs: Self) -> Self {
            // SAFETY: The new number can't exceed the input number of bits.
            Self::from_unchecked($operation(self.get(), rhs.get()))
        }
    };
}
use {operator_fn, operator_fn_closed, shift_fn};

impl DebugDisplay for InlineInt {
    fn fmt(&self, f: &mut Formatter, _is_debug: bool) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}
impl_debug_display_via_debugdisplay!(InlineInt);

impl_eq_hash_ord_via_get!(InlineInt);

impl TryFrom<&BigInt> for InlineInt {
    type Error = ();

    fn try_from(value: &BigInt) -> Result<Self, Self::Error> {
        i64::try_from(value)
            .map_err(|_| ())
            .and_then(TryInto::try_into)
    }
}
impl TryFrom<i64> for InlineInt {
    type Error = ();

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if Self::fits(value) {
            Ok(Self::from_unchecked(value))
        } else {
            Err(())
        }
    }
}

impl InlineObjectTrait for InlineInt {
    fn clone_to_heap_with_mapping(
        self,
        _heap: &mut Heap,
        _address_map: &mut FxHashMap<HeapObject, HeapObject>,
    ) -> Self {
        self
    }
}
impl_ord_with_symbol_table_via_ord!(InlineInt);

#[extension_trait]
pub impl I64BitLength for i64 {
    fn bit_length(self) -> u32 {
        Self::BITS - self.unsigned_abs().leading_zeros()
    }
}

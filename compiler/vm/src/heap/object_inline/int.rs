use super::{InlineObject, InlineObjectTrait};
use crate::{
    heap::{object_heap::HeapObject, Heap, Int, Tag},
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
    ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Sub},
};

#[derive(Clone, Copy, Deref)]
pub struct InlineInt(InlineObject);
impl InlineInt {
    const VALUE_SHIFT: usize = 3;
    pub const VALUE_BITS: usize = InlineObject::BITS as usize - Self::VALUE_SHIFT;

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

    pub fn compare_to(self, heap: &Heap, rhs: Int) -> Tag {
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
        Tag::create_ordering(heap, ordering)
    }

    pub fn shift_left(self, heap: &mut Heap, rhs: Self) -> Int {
        let lhs = self.get();
        #[allow(clippy::map_unwrap_or)]
        rhs.try_get::<u32>()
            .and_then(|rhs| {
                // `checked_shl(…)` only checks that `rhs` doesn't exceed the number of bits in the
                // type (i.e., `rhs < 64`). However, we need to check that the mathematical result
                // is completely representable in our available bits and doesn't get truncated.

                #[allow(clippy::cast_possible_truncation)]
                let value_shift = Self::VALUE_SHIFT as u32;

                if self.get().bit_length() + rhs < InlineObject::BITS - value_shift {
                    Some(lhs << rhs)
                } else {
                    None
                }
            })
            .filter(|it| it.signum() == lhs.signum())
            .map(|it| Int::create(heap, true, it))
            .unwrap_or_else(|| {
                Int::create_from_bigint(heap, true, BigInt::from(lhs).shl(rhs.get()))
            })
    }
    pub fn shift_right(self, rhs: Self) -> Self {
        // SAFETY: The value can only get closer to zero, so it must be covered by our range as
        // well.
        Self::from_unchecked(self.get() >> rhs.get())
    }

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
macro_rules! operator_fn_closed {
    ($name:ident, $operation:expr) => {
        pub fn $name(self, rhs: Self) -> Self {
            // SAFETY: The new number can't exceed the input number of bits.
            Self::from_unchecked($operation(self.get(), rhs.get()))
        }
    };
}
use {operator_fn, operator_fn_closed};

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

#[extension_trait]
pub impl I64BitLength for i64 {
    fn bit_length(self) -> u32 {
        if self.is_negative() {
            // One 1 is necessary for the sign.
            Self::BITS - self.leading_ones() + 1
        } else {
            Self::BITS - self.leading_zeros()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InlineInt;
    use crate::heap::{Heap, InlineObject, Int};
    use num_bigint::BigInt;

    #[test]
    fn shift_left() {
        let mut heap = Heap::default();
        assert_eq!(
            inline_int(1).shift_left(&mut heap, inline_int(1)),
            Int::create(&mut heap, true, 2),
        );
        assert_eq!(
            inline_int(2).shift_left(&mut heap, inline_int(1)),
            Int::create(&mut heap, true, 4),
        );
        assert_eq!(
            inline_int(-1).shift_left(&mut heap, inline_int(1)),
            Int::create(&mut heap, true, -2),
        );
        assert_eq!(
            inline_int(-2).shift_left(&mut heap, inline_int(1)),
            Int::create(&mut heap, true, -4),
        );

        {
            let shift = InlineInt::VALUE_BITS;
            #[allow(clippy::cast_possible_wrap)]
            let shift_inline_int = inline_int(shift as i64);
            assert_eq!(
                inline_int(1).shift_left(&mut heap, shift_inline_int),
                Int::create(&mut heap, true, 1i64 << shift),
            );
        }

        {
            let shift = InlineObject::BITS;
            #[allow(clippy::cast_lossless)]
            let shift_inline_int = inline_int(shift as i64);
            assert_eq!(
                inline_int(1).shift_left(&mut heap, shift_inline_int),
                Int::create_from_bigint(&mut heap, true, BigInt::from(1) << shift),
            );
        }
    }
    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn shift_right() {
        assert_eq!(inline_int(1).shift_right(inline_int(1)), inline_int(0));
        assert_eq!(inline_int(-1).shift_right(inline_int(1)), inline_int(-1));
        assert_eq!(inline_int(2).shift_right(inline_int(1)), inline_int(1));
        assert_eq!(inline_int(-2).shift_right(inline_int(1)), inline_int(-1));

        {
            // The leftmost bit must be zero since the number is positive.
            let shift = InlineInt::VALUE_BITS - 2;
            assert_eq!(
                inline_int(1 << shift).shift_right(inline_int(shift as i64)),
                inline_int(1),
            );
        }

        {
            let shift = inline_int(InlineInt::VALUE_BITS as i64 + 1);
            assert_eq!(inline_int(1).shift_right(shift), inline_int(0));
            assert_eq!(inline_int(-1).shift_right(shift), inline_int(-1));
        }
    }

    #[test]
    fn bit_length() {
        assert_eq!(inline_int(-4).bit_length(), inline_int(3));
        assert_eq!(inline_int(-3).bit_length(), inline_int(3));
        assert_eq!(inline_int(-2).bit_length(), inline_int(2));
        assert_eq!(inline_int(-1).bit_length(), inline_int(1));
        assert_eq!(inline_int(0).bit_length(), inline_int(0));
        assert_eq!(inline_int(1).bit_length(), inline_int(1));
        assert_eq!(inline_int(2).bit_length(), inline_int(2));
        assert_eq!(inline_int(3).bit_length(), inline_int(2));
        assert_eq!(inline_int(4).bit_length(), inline_int(3));
    }

    fn inline_int(value: i64) -> InlineInt {
        InlineInt::try_from(value).unwrap()
    }
}

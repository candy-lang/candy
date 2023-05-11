macro_rules! heap_object_impls {
    ($type:ty) => {
        impl<'h> TryFrom<$crate::heap::object_heap::HeapObject<'h>> for $type {
            type Error = &'static str;

            fn try_from(
                value: $crate::heap::object_heap::HeapObject<'h>,
            ) -> Result<Self, Self::Error> {
                $crate::heap::object_heap::HeapData::from(value).try_into()
            }
        }

        impl<'h> From<$type> for $crate::heap::object_heap::HeapObject<'h> {
            fn from(value: $type) -> Self {
                *value
            }
        }
        impl<'h> From<$type> for $crate::heap::object_inline::InlineObject<'h> {
            fn from(value: $type) -> Self {
                (*value).into()
            }
        }
    };
}
pub(super) use heap_object_impls;

use extension_trait::extension_trait;

#[extension_trait]
pub impl RefCountToString for Option<usize> {
    fn ref_count_to_string(&self) -> String {
        self.as_ref().map_or_else(
            || "not ref-counted".to_string(),
            |count| format!("{count} {}", if *count == 1 { "ref" } else { "refs" }),
        )
    }
}

macro_rules! heap_object_impls {
    ($type:ty) => {
        impl TryFrom<$crate::heap::object_heap::HeapObject> for $type {
            type Error = &'static str;

            fn try_from(value: $crate::heap::object_heap::HeapObject) -> Result<Self, Self::Error> {
                $crate::heap::object_heap::HeapData::from(value).try_into()
            }
        }

        impl From<$type> for $crate::heap::object_heap::HeapObject {
            fn from(value: $type) -> Self {
                *value
            }
        }
        impl From<$type> for $crate::heap::object_inline::InlineObject {
            fn from(value: $type) -> Self {
                (*value).into()
            }
        }
    };
}
pub(super) use heap_object_impls;

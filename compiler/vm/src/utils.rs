use std::fmt::{self, Debug, Display, Formatter};

pub trait DebugDisplay: Debug + Display {
    fn to_string(&self, is_debug: bool) -> String {
        if is_debug {
            format!("{:?}", self)
        } else {
            format!("{}", self)
        }
    }
    fn fmt(&self, f: &mut Formatter, is_debug: bool) -> fmt::Result;
}
macro_rules! impl_debug_display_via_debugdisplay {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                DebugDisplay::fmt(self, f, true)
            }
        }
        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                DebugDisplay::fmt(self, f, false)
            }
        }
    };
}

macro_rules! impl_eq_hash_via_get {
    ($type:ty) => {
        impl Eq for $type {}
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                self.get() == other.get()
            }
        }

        impl std::hash::Hash for $type {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.get().hash(state)
            }
        }
    };
}

pub(super) use {impl_debug_display_via_debugdisplay, impl_eq_hash_via_get};

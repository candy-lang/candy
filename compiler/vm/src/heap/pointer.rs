use std::fmt::Display;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pointer(usize);

impl Pointer {
    #[must_use]
    pub const fn null() -> Self {
        Self(0)
    }
    #[must_use]
    pub const fn from_raw(raw: usize) -> Self {
        Self(raw)
    }
    #[must_use]
    pub const fn raw(&self) -> usize {
        self.0
    }
}
impl Display for Pointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:X}", self.0)
    }
}

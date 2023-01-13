use std::fmt::Display;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pointer(usize);

impl Pointer {
    pub fn null() -> Self {
        Self(0)
    }
    pub fn from_raw(raw: usize) -> Self {
        Self(raw)
    }
    pub fn raw(&self) -> usize {
        self.0
    }
}
impl Display for Pointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

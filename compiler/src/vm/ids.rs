use std::{fmt, marker::PhantomData};

#[derive(Clone)]
pub struct IdGenerator<T: CountableId> {
    next_id: usize,
    _data: PhantomData<T>,
}
impl<T: CountableId> IdGenerator<T> {
    pub fn start_at(id: usize) -> Self {
        Self {
            next_id: id,
            _data: Default::default(),
        }
    }
    pub fn generate(&mut self) -> T {
        let id = self.next_id;
        self.next_id += 1;
        T::from_usize(id)
    }
}
pub trait CountableId {
    fn from_usize(id: usize) -> Self;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FiberId(usize);
impl CountableId for FiberId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
}
impl fmt::Debug for FiberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fiber_{:x}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(usize);

impl CountableId for ChannelId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
}
impl fmt::Debug for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "channel_{:x}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationId(usize);
impl CountableId for OperationId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
}
impl fmt::Debug for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "operation_{:x}", self.0)
    }
}

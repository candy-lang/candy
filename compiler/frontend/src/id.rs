use std::marker::PhantomData;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IdGenerator<T: CountableId> {
    next_id: usize,
    _data: PhantomData<T>,
}

impl<T: CountableId> IdGenerator<T> {
    #[must_use]
    pub const fn start_at(id: usize) -> Self {
        Self {
            next_id: id,
            _data: PhantomData,
        }
    }
    #[must_use]
    pub fn generate(&mut self) -> T {
        let id = self.next_id;
        self.next_id += 1;
        T::from_usize(id)
    }
}

impl<T: CountableId> Default for IdGenerator<T> {
    fn default() -> Self {
        Self::start_at(0)
    }
}

pub trait CountableId {
    #[must_use]
    fn from_usize(id: usize) -> Self;
    #[must_use]
    fn to_usize(&self) -> usize;
}

#[macro_export]
macro_rules! impl_countable_id {
    ($name:ident) => {
        impl $crate::id::CountableId for $name {
            fn from_usize(id: usize) -> Self {
                Self(id)
            }
            fn to_usize(&self) -> usize {
                self.0
            }
        }
    };
}

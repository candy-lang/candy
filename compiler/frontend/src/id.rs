use std::marker::PhantomData;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
impl<T: CountableId> Default for IdGenerator<T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            _data: Default::default(),
        }
    }
}

pub trait CountableId {
    fn from_usize(id: usize) -> Self;
    fn to_usize(&self) -> usize;
}

use std::marker::PhantomData;

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
    fn to_usize(&self) -> usize;
}

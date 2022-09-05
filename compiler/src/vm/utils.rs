use std::marker::PhantomData;

#[derive(Clone)]
pub struct IdGenerator<T: From<usize>> {
    next_id: usize,
    _data: PhantomData<T>,
}
impl<T: From<usize>> IdGenerator<T> {
    pub fn start_at(id: usize) -> Self {
        Self {
            next_id: id,
            _data: Default::default(),
        }
    }
    pub fn generate(&mut self) -> T {
        let id = self.next_id;
        self.next_id += 1;
        id.into()
    }
}

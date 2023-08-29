use rustc_hash::FxHashMap;
use std::{hash::Hash, num::NonZeroUsize};

// In some places (e.g., `Variable::variables_reference`), `0` is used to
// represent no value. (Not sure why they didn't use `null` like in many other
// places.) Therefore, the ID is the index in `keys` plus one.
pub struct IdMapping<T: Clone + Eq + Hash> {
    keys: Vec<T>,
    key_to_id: FxHashMap<T, NonZeroUsize>,
}

impl<T: Clone + Eq + Hash> IdMapping<T> {
    pub fn id_to_key(&self, id: NonZeroUsize) -> &T {
        &self.keys[id.get() - 1]
    }
    pub fn key_to_id(&mut self, key: T) -> NonZeroUsize {
        *self.key_to_id.entry(key.clone()).or_insert_with(|| {
            self.keys.push(key);
            self.keys.len().try_into().unwrap()
        })
    }
}

impl<T: Clone + Eq + Hash> Default for IdMapping<T> {
    fn default() -> Self {
        Self {
            keys: vec![],
            key_to_id: FxHashMap::default(),
        }
    }
}

use extension_trait::extension_trait;
use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash},
};

#[extension_trait]
pub impl<T, S> HashSetExtension<T> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn force_insert(&mut self, value: T) {
        assert!(self.insert(value));
    }
}

#[extension_trait]
pub impl<K, V, S> HashMapExtension<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn force_insert(&mut self, k: K, v: V) {
        assert!(self.insert(k, v).is_none());
    }
}

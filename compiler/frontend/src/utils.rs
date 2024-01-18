use extension_trait::extension_trait;
use rustc_hash::FxHasher;
use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasher, Hash, Hasher},
};

#[extension_trait]
pub impl AdjustCasingOfFirstLetter for str {
    fn lowercase_first_letter(&self) -> String {
        let mut c = self.chars();
        c.next().map_or_else(String::new, |f| {
            f.to_lowercase().collect::<String>() + c.as_str()
        })
    }

    fn uppercase_first_letter(&self) -> String {
        let mut c = self.chars();
        c.next().map_or_else(String::new, |f| {
            f.to_uppercase().collect::<String>() + c.as_str()
        })
    }
}

#[extension_trait]
pub impl<T: Hash> DoHash for T {
    fn do_hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[extension_trait]
pub impl<T> VecRetainIndexed<T> for Vec<T> {
    fn retain_indexed(&mut self, mut f: impl FnMut(usize, &T) -> bool) {
        let mut index = 0;
        self.retain(|item| {
            let keep = f(index, item);
            index += 1;
            keep
        });
    }
}

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

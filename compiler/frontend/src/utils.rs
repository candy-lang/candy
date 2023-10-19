use extension_trait::extension_trait;
use rustc_hash::FxHasher;
use std::{
    collections::HashMap,
    hash::{BuildHasher, BuildHasherDefault, Hash, Hasher},
};

pub type RcImHashSet<T> = im_rc::HashSet<T, BuildHasherDefault<FxHasher>>;
pub type RcImHashMap<K, V> = im_rc::HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type ArcImHashSet<T> = im::HashSet<T, BuildHasherDefault<FxHasher>>;
pub type ArcImHashMap<K, V> = im::HashMap<K, V, BuildHasherDefault<FxHasher>>;

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
pub impl<K, V, S> HashMapExtension<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn force_insert(&mut self, k: K, v: V) {
        assert!(self.insert(k, v).is_none());
    }
}

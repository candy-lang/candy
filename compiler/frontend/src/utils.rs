use extension_trait::extension_trait;
use rustc_hash::FxHasher;
use std::{
    collections::{HashMap, HashSet},
    hash::{BuildHasher, BuildHasherDefault, Hash, Hasher},
};

pub type RcImHashSet<T, S = BuildHasherDefault<FxHasher>> = im_rc::HashSet<T, S>;
pub type RcImHashMap<K, V, S = BuildHasherDefault<FxHasher>> = im_rc::HashMap<K, V, S>;
pub type ArcImHashSet<T, S = BuildHasherDefault<FxHasher>> = im::HashSet<T, S>;
pub type ArcImHashMap<K, V, S = BuildHasherDefault<FxHasher>> = im::HashMap<K, V, S>;

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
pub impl<T, S> HashSetExtension<T, S> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn force_insert(&mut self, value: T) {
        assert!(self.insert(value));
    }
    fn force_remove(&mut self, value: &T) {
        assert!(self.remove(value));
    }
}
#[extension_trait]
pub impl<K, V, S> HashMapExtension<K, V, S> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn force_insert(&mut self, k: K, v: V) {
        assert!(self.insert(k, v).is_none());
    }
    fn force_replace(&mut self, k: K, v: V) -> V {
        self.insert(k, v).unwrap()
    }
    fn force_remove(&mut self, k: &K) -> V {
        self.remove(k).unwrap()
    }
}

macro_rules! impl_im_force_insert {
    ($name:ident for $hash_map_type:ident) => {
        #[extension_trait]
        pub impl<K, V, S> $name<K, V, S> for $hash_map_type<K, V, S>
        where
            K: Clone + Eq + Hash,
            V: Clone,
            S: BuildHasher,
        {
            fn force_insert(&mut self, k: K, v: V) {
                assert!(self.insert(k, v).is_none());
            }
            fn force_replace(&mut self, k: K, v: V) -> V {
                self.insert(k, v).unwrap()
            }
            fn force_remove(&mut self, k: &K) -> V {
                self.remove(k).unwrap()
            }
        }
    };
}
impl_im_force_insert!(RcImHashMapExtension for RcImHashMap);
impl_im_force_insert!(ArcImHashMapExtension for ArcImHashMap);

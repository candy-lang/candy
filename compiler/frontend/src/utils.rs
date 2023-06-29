use extension_trait::extension_trait;
use rustc_hash::FxHasher;
use std::hash::{BuildHasherDefault, Hash, Hasher};

pub type ImHashSet<T> = im_rc::HashSet<T, BuildHasherDefault<FxHasher>>;
pub type ImHashMap<K, V> = im_rc::HashMap<K, V, BuildHasherDefault<FxHasher>>;

#[extension_trait]
pub impl AdjustCasingOfFirstLetter for str {
    fn lowercase_first_letter(&self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        }
    }

    fn uppercase_first_letter(&self) -> String {
        let mut c = self.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
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

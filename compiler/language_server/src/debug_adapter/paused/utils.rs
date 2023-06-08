use candy_vm::{
    fiber::{Fiber, FiberId},
    lir::Lir,
    tracer::Tracer,
    vm::Vm,
};
use extension_trait::extension_trait;
use rustc_hash::FxHashMap;
use std::{borrow::Borrow, hash::Hash};

#[extension_trait]
pub impl FiberIdExtension for FiberId {
    fn get<'a, 'c: 'h, 'h, L: Borrow<Lir<'c>>, T: Tracer<'h>>(
        self,
        vm: &'a Vm<'c, 'h, L, T>,
    ) -> &'a Fiber<'c, 'h, T::ForFiber> {
        vm.fiber(self).unwrap().fiber_ref()
    }
}

// In some places (e.g., `Variable::variables_reference`), `0` is used to
// represent no value. (Not sure why they didn't use `null` like in many other
// places.) Therefore, the ID is the index in `keys` plus one.
pub struct IdMapping<T: Clone + Eq + Hash> {
    keys: Vec<T>,
    key_to_id: FxHashMap<T, i64>, // FIXME: NonZeroI64
}

impl<T: Clone + Eq + Hash> IdMapping<T> {
    pub fn id_to_key(&self, id: i64) -> &T {
        &self.keys[(id - 1) as usize]
    }
    pub fn key_to_id(&mut self, key: T) -> i64 {
        *self.key_to_id.entry(key.clone()).or_insert_with(|| {
            self.keys.push(key);
            self.keys.len() as i64
        })
    }
}

impl<T: Clone + Eq + Hash> Default for IdMapping<T> {
    fn default() -> Self {
        Self {
            keys: vec![],
            key_to_id: Default::default(),
        }
    }
}

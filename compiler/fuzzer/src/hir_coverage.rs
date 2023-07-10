use candy_frontend::hir::Id;
use rustc_hash::FxHashSet;
use std::{fmt, ops::Add};

// TODO: Be more efficient by saving IDs as a tree.
#[derive(Clone)]
pub struct HirCoverage(FxHashSet<Id>);

impl HirCoverage {
    pub fn none() -> Self {
        Self(FxHashSet::default())
    }

    pub fn add(&mut self, id: Id) {
        self.0.insert(id);
    }

    pub fn all_ids(&self) -> impl Iterator<Item = &Id> {
        self.0.iter()
    }
}
impl Add for &HirCoverage {
    type Output = HirCoverage;

    fn add(self, rhs: Self) -> Self::Output {
        let mut covered = self.0.clone();
        covered.extend(rhs.0.iter().cloned());
        HirCoverage(covered)
    }
}

impl fmt::Debug for HirCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for id in &self.0 {
            write!(f, "{}", id)?;
        }
        write!(f, "]")
    }
}

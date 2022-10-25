mod common_subtree_elimination;
mod complexity;
mod constant_folding;
mod follow_references;
mod inlining;
mod module_folding;
mod tree_shaking;
mod utils;

use super::mir::Mir;
use crate::{database::Database, module::Module};
use tracing::debug;

impl Mir {
    pub fn optimize(&mut self, db: &Database) {
        debug!("MIR: {self:?}");
        debug!("Complexity: {}", self.complexity());
        self.optimize_obvious(db, &[]);
        debug!("Done optimizing.");
        debug!("MIR: {self:?}");
        debug!("Complexity: {}", self.complexity());
    }

    /// Performs optimizations without negative effects.
    pub fn optimize_obvious(&mut self, db: &Database, import_chain: &[Module]) {
        self.optimize_obvious_self_contained();
        self.fold_modules(db, import_chain);
        self.optimize_obvious_self_contained();
    }

    /// Performs optimizations without negative effects that work without
    /// looking at other modules.
    pub fn optimize_obvious_self_contained(&mut self) {
        loop {
            let before = self.clone();

            self.follow_references();
            self.tree_shake();
            self.fold_constants();
            self.inline_functions_containing_use();
            self.eliminate_common_subtrees();

            if *self == before {
                return;
            }
        }
    }
}

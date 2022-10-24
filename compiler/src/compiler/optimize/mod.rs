// mod constant_folding;
// mod inlining;
// mod constant_folding;
// mod inlining;
// mod module_folding;
// mod tree_shaking;
mod complexity;
mod follow_references;
mod utils;

use super::mir::Mir;
use crate::{database::Database, module::Module};
use tracing::warn;

impl Mir {
    pub fn optimize(&mut self, db: &Database) {
        warn!("MIR: {self:?}");
        warn!("Complexity: {}", self.complexity());
        self.optimize_obvious(db, &[]);
        warn!("Done optimizing.");
        // warn!("Complexity: {}", self.complexity());
        // self.tree_shake();
        // warn!("Complexity: {}", self.complexity());
        // warn!("Following references");
        // self.follow_references();
        // warn!("MIR: {self:?}");
        // warn!("Tree shaking");
        // self.tree_shake();
        // warn!("MIR: {self:?}");
        // warn!("Folding constants");
        // self.fold_constants();
        // warn!("MIR: {self:?}");
        // warn!("Inlining");
        // inline HirId(user:"/home/marcel/projects/candy":packages/Benchmark:78)
        // let call = self.ids[2].clone();
        // let result = self.inline_call(&call);
        // self.inline_functions_containing_use();
        // self.tree_shake();
        // warn!("{result:?}");
        warn!("MIR: {self:?}");
    }

    /// Performs optimizations without negative effects.
    pub fn optimize_obvious(&mut self, db: &Database, import_chain: &[Module]) {
        warn!("Optimizing obvious. Import chain: {import_chain:?}");
        self.optimize_obvious_self_contained();
        // warn!("MIR: {self:?}");
        warn!("Folding modules");
        // warn!("MIR: {self:?}");
        // self.fold_modules(db, import_chain);
        self.optimize_obvious_self_contained();
    }

    /// Performs optimizations without negative effects that work without
    /// looking at other modules.
    pub fn optimize_obvious_self_contained(&mut self) {
        loop {
            let before = self.clone();

            warn!("Optimizing self-contained obvious things");
            warn!("Following references");
            self.follow_references();
            warn!("Still the same? {}", *self == before);
            // warn!("Tree shaking");
            // self.tree_shake();
            // warn!("Still the same? {}", *self == before);
            // warn!("Folding constants");
            // self.fold_constants();
            // warn!("Still the same? {}", *self == before);
            // warn!("Inlining functions containing use");
            // self.inline_functions_containing_use();
            // warn!("Still the same? {}", *self == before);
            // warn!("Complexity: {}", self.complexity());

            if *self == before {
                return;
            }
        }
    }
}

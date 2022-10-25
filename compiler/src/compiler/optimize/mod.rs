// mod module_folding;
mod complexity;
mod constant_folding;
mod follow_references;
mod tree_shaking;
mod utils;

use super::mir::Mir;
use crate::{database::Database, module::Module};
use tracing::{debug, warn};

impl Mir {
    pub fn optimize(&mut self, db: &Database) {
        debug!("MIR: {self:?}");
        debug!("Complexity: {}", self.complexity());
        self.optimize_obvious(db, &[]);
        debug!("Done optimizing.");
        // debug!("Complexity: {}", self.complexity());
        // self.tree_shake();
        // debug!("Complexity: {}", self.complexity());
        // debug!("Following references");
        // self.follow_references();
        // debug!("MIR: {self:?}");
        // debug!("Tree shaking");
        // self.tree_shake();
        // debug!("MIR: {self:?}");
        // debug!("Folding constants");
        // self.fold_constants();
        // debug!("MIR: {self:?}");
        // debug!("Inlining");
        // inline HirId(user:"/home/marcel/projects/candy":packages/Benchmark:78)
        // let call = self.ids[2].clone();
        // let result = self.inline_call(&call);
        // self.inline_functions_containing_use();
        // self.tree_shake();
        // debug!("{result:?}");
        debug!("MIR: {self:?}");
    }

    /// Performs optimizations without negative effects.
    pub fn optimize_obvious(&mut self, db: &Database, import_chain: &[Module]) {
        debug!("Optimizing obvious. Import chain: {import_chain:?}");
        self.optimize_obvious_self_contained();
        // debug!("MIR: {self:?}");
        debug!("Folding modules");
        // debug!("MIR: {self:?}");
        // self.fold_modules(db, import_chain);
        self.optimize_obvious_self_contained();
    }

    /// Performs optimizations without negative effects that work without
    /// looking at other modules.
    pub fn optimize_obvious_self_contained(&mut self) {
        loop {
            let before = self.clone();

            debug!("Optimizing self-contained obvious things");
            self.follow_references();
            self.tree_shake();
            self.fold_constants();
            self.inline_functions_containing_use();

            debug!("Complexity: {}", self.complexity());

            if *self == before {
                return;
            }
        }
    }
}

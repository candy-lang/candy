// mod constant_folding;
// mod inlining;
mod constant_folding;
mod follow_references;
mod tree_shaking;
mod utils;

use super::hir::Body;
use tracing::{debug, info, warn};

impl Body {
    pub fn optimize(&mut self) {
        warn!("HIR: {self}");
        warn!("Following references");
        self.follow_references();
        warn!("HIR: {self}");
        warn!("Tree shaking");
        self.tree_shake();
        warn!("HIR: {self}");
        // self.fold_constants();
        warn!("HIR: {self}");
    }
}

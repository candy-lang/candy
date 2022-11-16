//! Module folding evaluates imports with known argument at compile-time.
//!
//! This is similar to [constant folding], but for the `builtinUseModule`
//! builtin. This is also similar to [inlining], but for entire module contents.
//!
//! Here's a before-and-after example of an import of Core being folded:
//!
//! ```mir
//! # before:
//! $0 = "Core"
//! $1 = HirId(the `use "Core"` expression)
//! $2 = use $0 relative to here, $1 responsible
//!
//! # after:
//! $0 = "Core"
//! $1 = HirId(the `use "Core"` expression)
//! $2 =
//!   (code of Core)
//! ```
//!
//! Like [inlining], module folding enables many other optimizations, but across
//! module boundaries. If all imports can be resolved at compile-time, that also
//! means that the VM never needs to interrupt the program execution for parsing
//! and compiling other modules. Module folding is a necessity for building
//! binaries that don't include the Candy compiler itself.
//!
//! [constant folding]: super::constant_folding
//! [inlining]: super::inlining

use crate::{
    compiler::{
        hir_to_mir::TracingConfig,
        mir::{Expression, Id, Mir},
        optimize::OptimizeMir,
    },
    module::UsePath,
};
use std::collections::HashMap;
use tracing::warn;

impl Mir {
    pub fn fold_modules(&mut self, db: &dyn OptimizeMir, config: &TracingConfig) {
        self.body
            .visit_with_visible(&mut |_, expression, visible, _| {
                let Expression::UseModule {
                    current_module,
                    relative_path,
                    responsible: _,
                } = expression else { return; };

                let Expression::Text(path) = visible.get(*relative_path) else {
                // warn!("use called with non-constant text");
                return; // TODO
            };
                let Ok(path) = UsePath::parse(path) else {
                warn!("use called with an invalid path");
                return; // TODO
            };
                let Ok(module_to_import) = path.resolve_relative_to(current_module.clone()) else {
                warn!("use called with an invalid path");
                return; // TODO
            };

                // debug!("Loading and optimizing module {module_to_import}");
                let mir = db.mir_with_obvious_optimized(module_to_import, config.clone());
                let Some(mir) = mir else {
                warn!("Module not found.");
                return; // TODO
            };
                let mir = (*mir).clone();

                let mapping: HashMap<Id, Id> = mir
                    .body
                    .all_ids()
                    .into_iter()
                    .map(|id| (id, self.id_generator.generate()))
                    .collect();
                let mut body_to_insert = mir.body;
                body_to_insert.replace_ids(&mut |id| *id = mapping[id]);

                *expression = Expression::Multiple(body_to_insert);
            });
    }
}

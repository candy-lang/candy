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
    error::CompilerError,
    id::IdGenerator,
    mir::{Body, Expression, Id, Mir, MirError, VisitorResult},
    mir_optimize::OptimizeMir,
    module::{Module, UsePath},
    tracing::TracingConfig,
};
use rustc_hash::{FxHashMap, FxHashSet};

impl Mir {
    pub fn fold_modules(
        &mut self,
        db: &dyn OptimizeMir,
        tracing: &TracingConfig,
        errors: &mut FxHashSet<CompilerError>,
    ) {
        self.body
            .visit_with_visible(&mut |_, expression, visible, _| {
                let Expression::UseModule {
                    current_module,
                    relative_path,
                    responsible,
                } = expression else { return; };

                let Expression::Text(path) = visible.get(*relative_path) else {
                    *expression = panicking_expression(&mut self.id_generator, "`use` expects a text as a path.".to_string(), *responsible);
                    return;
                };

                let module_to_import = match Self::resolve_module(current_module, path) {
                    Ok(module) => module,
                    Err(error) => {
                        let error = CompilerError::for_whole_module(current_module.clone(), error);
                        *expression = panicking_expression(&mut self.id_generator, error.payload.to_string(), *responsible);
                        errors.insert(error);
                        return;
                    },
                };

                let (mir, more_errors) = db.optimized_mir(module_to_import, tracing.for_child_module());
                errors.extend((*more_errors).clone().into_iter());

                let mapping: FxHashMap<Id, Id> = mir
                    .body
                    .all_ids()
                    .into_iter()
                    .map(|id| (id, self.id_generator.generate()))
                    .collect();
                let mut body_to_insert = mir.body.clone();
                body_to_insert.replace_ids(&mut |id| *id = mapping[&*id]);

                *expression = Expression::Multiple(body_to_insert);
            });
    }
    fn resolve_module(current_module: &Module, path: &str) -> Result<Module, MirError> {
        let Ok(path) = UsePath::parse(path) else {
            return Err(MirError::UseWithInvalidPath { module: current_module.clone(), path: path.to_string() });
        };
        let Ok(module) = path.resolve_relative_to(current_module.clone()) else {
            return Err(MirError::UseHasTooManyParentNavigations { module: current_module.clone(), path: path.to_string() });
        };
        Ok(module)
    }

    pub fn replace_remaining_uses_with_panics(&mut self, errors: &mut FxHashSet<CompilerError>) {
        self.body.visit(&mut |_, expression, _| {
            if let Expression::UseModule {
                current_module,
                responsible,
                ..
            } = expression
            {
                let error = CompilerError::for_whole_module(
                    current_module.clone(),
                    MirError::UseNotStaticallyResolvable {
                        containing_module: current_module.clone(),
                    },
                );
                *expression = panicking_expression(
                    &mut self.id_generator,
                    error.payload.to_string(),
                    *responsible,
                );
                errors.insert(error);
            }
            VisitorResult::Continue
        });
    }
}

fn panicking_expression(
    id_generator: &mut IdGenerator<Id>,
    reason: String,
    responsible: Id,
) -> Expression {
    let mut body = Body::default();
    let reason = body.push_with_new_id(id_generator, Expression::Text(reason));
    body.push_with_new_id(
        id_generator,
        Expression::Panic {
            reason,
            responsible,
        },
    );

    Expression::Multiple(body)
}

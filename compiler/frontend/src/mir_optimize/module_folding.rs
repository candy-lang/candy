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

use super::current_expression::ExpressionContext;
use crate::{
    error::{CompilerError, CompilerErrorPayload},
    id::IdGenerator,
    mir::{Body, BodyBuilder, Expression, Id, MirError},
    mir_optimize::OptimizeMir,
    module::{Module, UsePath},
    tracing::TracingConfig,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::mem;

pub fn apply(
    context: &mut ExpressionContext,
    db: &dyn OptimizeMir,
    tracing: &TracingConfig,
    errors: &mut FxHashSet<CompilerError>,
) {
    let Expression::UseModule { current_module, relative_path, responsible } = &*context.expression else {
        return;
    };
    let responsible = *responsible;

    let path = match context.visible.get(*relative_path) {
        Expression::Text(path) => path,
        Expression::Parameter => {
            // After optimizing, the MIR should no longer contain any `use`.
            // However, here we are in the `use` function. All calls to this
            // function are guaranteed to be inlined using the
            // `inlining::inline_functions_containing_use` optimization and
            // then, this function will be removed entirely. That's why it's
            // fine to leave the `use` here.
            return;
        }
        Expression::Call { .. } => {
            let error = CompilerError::for_whole_module(
                current_module.clone(),
                MirError::UseNotStaticallyResolvable {
                    containing_module: current_module.clone(),
                },
            );
            context
                .expression
                .replace_with_multiple(panicking_expression(
                    context.id_generator,
                    error.payload.to_string(),
                    responsible,
                ));
            errors.insert(error);
            return;
        }
        _ => {
            context
                .expression
                .replace_with_multiple(panicking_expression(
                    context.id_generator,
                    "`use` expects a text as a path.".to_string(),
                    responsible,
                ));
            return;
        }
    };

    let module_to_import = match resolve_module(current_module, path) {
        Ok(module) => module,
        Err(error) => {
            let error = CompilerError::for_whole_module(current_module.clone(), error);
            context
                .expression
                .replace_with_multiple(panicking_expression(
                    context.id_generator,
                    error.payload.to_string(),
                    responsible,
                ));
            errors.insert(error);
            return;
        }
    };

    match db.optimized_mir(module_to_import.clone(), tracing.for_child_module()) {
        Ok((mir, other_pureness, more_errors)) => {
            errors.extend(more_errors.iter().cloned());

            let mapping: FxHashMap<Id, Id> = mir
                .body
                .all_ids()
                .into_iter()
                .map(|id| (id, context.id_generator.generate()))
                .collect();

            context.pureness.include(other_pureness.as_ref(), &mapping);
            context.prepend_optimized(mir.body.iter().map(|(id, expression)| {
                let mut expression = expression.to_owned();
                expression.replace_ids(&mapping);
                (mapping[&id], expression)
            }));
            *context.expression = Expression::Reference(mapping[&mir.body.return_value()]);
        }
        Err(error) => {
            errors.insert(CompilerError::for_whole_module(module_to_import, error));

            let inner_id_generator = mem::take(context.id_generator);
            let mut builder = BodyBuilder::new(inner_id_generator);

            let reason = builder.push_text(CompilerErrorPayload::Module(error).to_string());
            builder.push_panic(reason, responsible);

            let (inner_id_generator, body) = builder.finish();
            *context.id_generator = inner_id_generator;
            context.expression.replace_with_multiple(body);
        }
    };
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

fn panicking_expression(
    id_generator: &mut IdGenerator<Id>,
    reason: String,
    responsible: Id,
) -> Vec<(Id, Expression)> {
    let mut body = Body::default();
    let reason = body.push_with_new_id(id_generator, Expression::Text(reason));
    body.push_with_new_id(
        id_generator,
        Expression::Panic {
            reason,
            responsible,
        },
    );
    body.expressions
}

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

use super::current_expression::{Context, CurrentExpression};
use crate::{
    error::{CompilerError, CompilerErrorPayload},
    hir_to_mir::ExecutionTarget,
    id::IdGenerator,
    mir::{Body, BodyBuilder, Expression, Id, MirError},
    module::{Module, UsePath},
};
use rustc_hash::FxHashMap;
use std::mem;

const NAME: &str = "Module Folding";

pub fn apply(context: &mut Context, expression: &mut CurrentExpression) {
    let Expression::UseModule {
        current_module,
        relative_path,
        responsible,
    } = &**expression
    else {
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
            expression.replace_with_multiple(
                NAME,
                panicking_expression(context.id_generator, error.payload.to_string(), responsible),
                context.pureness,
            );
            context.errors.insert(error);
            return;
        }
        _ => {
            expression.replace_with_multiple(
                NAME,
                panicking_expression(
                    context.id_generator,
                    "`use` expects a text as a path.".to_string(),
                    responsible,
                ),
                context.pureness,
            );
            return;
        }
    };

    let module_to_import = match resolve_module(current_module, path) {
        Ok(module) => module,
        Err(error) => {
            let error = CompilerError::for_whole_module(current_module.clone(), error);
            expression.replace_with_multiple(
                NAME,
                panicking_expression(context.id_generator, error.payload.to_string(), responsible),
                context.pureness,
            );
            context.errors.insert(error);
            return;
        }
    };

    match context.db.optimized_mir_without_tail_calls(
        ExecutionTarget::Module(module_to_import.clone()),
        context.tracing.for_child_module(),
    ) {
        Ok((mir, other_pureness, more_errors)) => {
            context.errors.extend(more_errors.iter().cloned());

            let mapping: FxHashMap<Id, Id> = mir
                .body
                .all_ids()
                .into_iter()
                .map(|id| (id, context.id_generator.generate()))
                .collect();

            context.pureness.include(other_pureness.as_ref(), &mapping);
            expression.prepend_optimized(
                NAME,
                context.visible,
                mir.body.iter().map(|(id, expression)| {
                    let mut expression = expression.clone();
                    expression.replace_ids(&mut |id| {
                        if let Some(new_id) = mapping.get(id) {
                            *id = *new_id;
                        }
                    });
                    (mapping[&id], expression)
                }),
            );
            expression.replace_with(
                NAME,
                Expression::Reference(mapping[&mir.body.return_value()]),
                context.pureness,
            );
        }
        Err(error) => {
            context
                .errors
                .insert(CompilerError::for_whole_module(module_to_import, error));

            let inner_id_generator = mem::take(context.id_generator);
            let mut builder = BodyBuilder::new(inner_id_generator);

            let reason = builder.push_text(CompilerErrorPayload::Module(error).to_string());
            builder.push_panic(reason, responsible);

            let (inner_id_generator, body) = builder.finish();
            *context.id_generator = inner_id_generator;
            expression.replace_with_multiple(NAME, body, context.pureness);
        }
    };
}

fn resolve_module(current_module: &Module, path: &str) -> Result<Module, MirError> {
    let Ok(path) = UsePath::parse(path) else {
        return Err(MirError::UseWithInvalidPath {
            module: current_module.clone(),
            path: path.to_string(),
        });
    };
    let Ok(module) = path.resolve_relative_to(current_module) else {
        return Err(MirError::UseHasTooManyParentNavigations {
            module: current_module.clone(),
            path: path.to_string(),
        });
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

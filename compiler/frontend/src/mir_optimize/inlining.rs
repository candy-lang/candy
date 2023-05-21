//! Inlining means inserting a function's code at the caller site.
//!
//! Here's a before-and-after example of a `use "Core"` call being inlined:
//!
//! ```mir
//! # before:
//! $0 = { $1 ($2 responsible) ->
//!   $3 = use $1 relative to here, $2 responsible
//! }
//! $4 = "Core"
//! $5 = HirId(the `use "Core"` expression)
//! $6 = call $0 with $4 ($5 is responsible)
//!
//! # after:
//! $0 = { $1 ($2 responsible) ->
//!   $3 = use $1 relative to here, $2 responsible
//! }
//! $4 = "Core"
//! $5 = HirId(the `use "Core"` expression)
//! $6 =
//!   $7 = use $4 relative to here, $5 responsible
//! ```
//!
//! Inlining makes lots of other optimizations more effective, in partiuclar
//! [tree shaking] of functions that were inlined into all call sites. Because
//! at the call sites, more information about arguments exist,
//! [constant folding] and [module folding] can be more effective.
//!
//! TODO: When we have a metric for judging performance vs. code size, also
//! speculatively inline more call sites, such as smallish functions and
//! functions only used once.
//!
//! [constant folding]: super::constant_folding
//! [module folding]: super::module_folding
//! [tree shaking]: super::tree_shaking

use rustc_hash::FxHashMap;

use crate::{
    id::IdGenerator,
    mir::{Expression, Id, VisibleExpressions},
};

use super::complexity::Complexity;

pub fn inline_tiny_functions(
    expression: &mut Expression,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
) {
    inline_functions_of_maximum_complexity(
        expression,
        Complexity {
            is_self_contained: true,
            expressions: 2,
        },
        visible,
        id_generator,
    );
}

pub fn inline_functions_of_maximum_complexity(
    expression: &mut Expression,
    complexity: Complexity,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
) {
    if let Expression::Call { function, .. } = expression
        && let Expression::Function { body, .. } = visible.get(*function)
        && body.complexity() <= complexity {
        let _ = expression.inline_call(visible, id_generator);
    }
}

pub fn inline_functions_containing_use(
    expression: &mut Expression,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
) {
    if let Expression::Call { function, .. } = expression
        && let Expression::Function { body, .. } = visible.get(*function)
        && body.iter().any(|(_, expr)| matches!(expr, Expression::UseModule { .. })) {
        let _ = expression.inline_call(visible, id_generator);
    }
}

impl Expression {
    pub fn inline_call(
        &mut self,
        visible: &VisibleExpressions,
        id_generator: &mut IdGenerator<Id>,
    ) -> Result<(), &'static str> {
        let Expression::Call {
            function,
            arguments,
            responsible: responsible_argument,
        } = self else {
            return Err("Tried to inline, but the expression is not a call.");
        };
        let Expression::Function {
            original_hirs: _,
            parameters,
            responsible_parameter,
            body,
        } = visible.get(*function) else {
            return Err("Tried to inline, but the callee is not a function.");
        };
        if arguments.len() != parameters.len() {
            return Err("Tried to inline, but the number of arguments doesn't match the expected parameter count.");
        }

        let id_mapping: FxHashMap<Id, Id> = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| (*parameter, *argument))
            .chain([(*responsible_parameter, *responsible_argument)])
            .chain(
                body.defined_ids()
                    .into_iter()
                    .map(|id| (id, id_generator.generate())),
            )
            .collect();
        let mut inlined_body = body.clone();
        inlined_body.replace_ids(&mut |id| {
            if let Some(replacement) = id_mapping.get(id) {
                *id = *replacement;
            }
        });

        *self = Expression::Multiple(inlined_body);

        Ok(())
    }
}

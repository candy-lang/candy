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
    mir::{Expression, Id},
};

use super::{complexity::Complexity, current_expression::ExpressionContext};

pub fn inline_tiny_functions(context: &mut ExpressionContext, id_generator: &mut IdGenerator<Id>) {
    inline_functions_of_maximum_complexity(
        context,
        Complexity {
            is_self_contained: true,
            expressions: 7,
        },
        id_generator,
    );
}

pub fn inline_functions_of_maximum_complexity(
    context: &mut ExpressionContext,
    complexity: Complexity,
    id_generator: &mut IdGenerator<Id>,
) {
    if let Expression::Call { function, .. } = *context.expression
        && let Expression::Function { body, .. } = context.visible.get(function)
        && body.complexity() <= complexity {
        let _ = context.inline_call(id_generator);
    }
}

pub fn inline_functions_containing_use(
    context: &mut ExpressionContext,
    id_generator: &mut IdGenerator<Id>,
) {
    if let Expression::Call { function, .. } = *context.expression
        && let Expression::Function { body, .. } = context.visible.get(function)
        && body.iter().any(|(_, expr)| matches!(expr, Expression::UseModule { .. })) {
        let _ = context.inline_call(id_generator);
    }
}

impl ExpressionContext<'_> {
    pub fn inline_call(&mut self, id_generator: &mut IdGenerator<Id>) -> Result<(), &'static str> {
        // FIXME: Remove return values as they're unused.
        let Expression::Call {
            function,
            arguments,
            responsible: responsible_argument,
        } = &*self.expression else {
            return Err("Tried to inline, but the expression is not a call.");
        };
        if arguments.contains(function) {
            return Err("Tried to inline, but the callee is used as an argument → recursion.");
        }

        let Expression::Function {
            original_hirs: _,
            parameters,
            responsible_parameter,
            body,
        } = self.visible.get(*function) else {
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

        self.expression
            .replace_with_multiple(body.iter().map(|(id, expression)| {
                let mut expression = expression.to_owned();
                expression.replace_ids(&id_mapping);
                (id_mapping[&id], expression)
            }));

        Ok(())
    }
}

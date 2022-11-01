//! Inlining means inserting a lambda's code at the caller site.
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
//! Inlining makes lots of other optimizations more effective, in partuclar
//! [tree shaking] of lambdas that were inlined into all call sites. Because at
//! the call sites, more information about arguments exist, [constant folding]
//! and [module folding] can be more effective.
//!
//! TODO: Also inline functions only used once.
//! TODO: Also inline small functions containing only one expression. That
//! doesn't even make the code longer, just replaces the call.
//! TODO: Speculatively inline smallish functions and do more optimizations to
//! see if they become obvious optimizations.
//! TODO: When we have a metric for judging performance vs. code size, also
//! speculatively inline more call sites.
//!
//! [constant folding]: super::constant_folding
//! [module folding]: super::module_folding
//! [tree shaking]: super::tree_shaking

use crate::{
    compiler::mir::{Expression, Id, Mir, VisibleExpressions},
    utils::IdGenerator,
};
use std::collections::{HashMap, HashSet};

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
        let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            fuzzable: _,
        } = visible.get(*function) else {
            return Err("Tried to inline, but the call's receiver is not a lambda.");
        };
        if arguments.len() != parameters.len() {
            return Err("Tried to inline, but the number of arguments doesn't match the expected parameter count.");
        }

        let id_mapping: HashMap<Id, Id> = parameters
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

impl Mir {
    pub fn inline_functions_containing_use(&mut self) {
        let mut functions_with_use = HashSet::new();
        for (id, expression) in self.body.iter() {
            if let Expression::Lambda { body, .. } = expression &&
                    body.iter().any(|(_, expr)| matches!(expr, Expression::UseModule { .. })) {
                    functions_with_use.insert(id);
                }
        }

        self.body.visit(&mut |_, expression, visible, _| {
            if let Expression::Call { function, .. } = expression && functions_with_use.contains(&function) {
                expression.inline_call(visible, &mut self.id_generator);
            }
        });
    }
}

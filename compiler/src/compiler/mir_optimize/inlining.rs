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
//! Inlining makes lots of other optimizations more effective, in partiuclar
//! [tree shaking] of lambdas that were inlined into all call sites. Because at
//! the call sites, more information about arguments exist, [constant folding]
//! and [module folding] can be more effective.
//!
//! TODO: When we have a metric for judging performance vs. code size, also
//! speculatively inline more call sites, such as smallish functions and
//! functions only used once.
//!
//! [constant folding]: super::constant_folding
//! [module folding]: super::module_folding
//! [tree shaking]: super::tree_shaking

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    compiler::mir::{Expression, Id, Mir, VisibleExpressions},
    utils::IdGenerator,
};

use super::complexity::Complexity;

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
        } = visible.get(*function) else {
            return Err("Tried to inline, but the callee is not a lambda.");
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

impl Mir {
    pub fn inline_functions_containing_use(&mut self) {
        let mut functions_with_use = FxHashSet::default();
        for (id, expression) in self.body.iter() {
            if let Expression::Lambda { body, .. } = expression &&
                    body.iter().any(|(_, expr)| matches!(expr, Expression::UseModule { .. })) {
                functions_with_use.insert(id);
            }
        }

        self.body.visit_with_visible(&mut |_, expression, visible, _| {
            if let Expression::Call { function, .. } = expression && functions_with_use.contains(function) {
                // If inlining fails with an `Err`, there's nothing we can do
                // except apply other optimizations first and then try again
                // later.
                let _ = expression.inline_call(visible, &mut self.id_generator);
            }
        });
    }

    pub fn inline_functions_of_maximum_complexity(&mut self, complexity: Complexity) {
        let mut small_functions = FxHashSet::default();
        for (id, expression) in self.body.iter() {
            if let Expression::Lambda { body, .. } = expression && body.complexity() <= complexity {
                small_functions.insert(id);
            }
        }

        self.body.visit_with_visible(&mut |_, expression, visible, _| {
            if let Expression::Call { function, .. } = expression && small_functions.contains(function) {
                let _ = expression.inline_call(visible, &mut self.id_generator);
            }
        });
    }

    pub fn inline_tiny_functions(&mut self) {
        self.inline_functions_of_maximum_complexity(Complexity {
            is_self_contained: true,
            expressions: 1,
        });
    }

    pub fn inline_functions_only_called_once(&mut self) {
        let mut reference_counts: FxHashMap<Id, usize> = FxHashMap::default();
        self.body.replace_id_references(&mut |id| {
            *reference_counts.entry(*id).or_default() += 1;
        });
        self.body.visit_with_visible(&mut |_, expression, visible, _| {
            if let Expression::Call { function, .. } = expression && reference_counts[function] == 1 {
                let _ = expression.inline_call(visible, &mut self.id_generator);
            }
        });
    }
}

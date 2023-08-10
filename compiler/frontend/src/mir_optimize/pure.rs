use crate::mir::{Expression, Id};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PurenessInsights {
    // TODO: Simplify to `FxHashSet<Id>`s.
    definition_pureness: FxHashMap<Id, bool>,
    definition_constness: FxHashMap<Id, bool>,
}
impl PurenessInsights {
    /// Whether the expression defined at the given ID is pure.
    ///
    /// E.g., a function definition is pure even if the defined function is not
    /// pure.
    #[allow(clippy::unused_self)]
    pub const fn is_definition_pure(&self, expression: &Expression) -> bool {
        match expression {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Tag { .. }
            | Expression::Builtin(_) // TODO: Check if the builtin is pure.
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::Reference(_)
            | Expression::HirId(_)
            | Expression::Function { .. }
            | Expression::Parameter => true,
            // TODO: Check whether executing the function with the given arguments is pure when we inspect data flow.
            Expression::Call { .. } | Expression::UseModule { .. } | Expression::Panic { .. } => {
                false
            }
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }
    /// Whether the value of this expression is pure and known at compile-time.
    ///
    /// This is useful for moving expressions around without changing the
    /// semantics.
    pub fn is_definition_const(&self, expression: &Expression) -> bool {
        self.is_definition_pure(expression)
            && expression.captured_ids().iter().all(|id| {
                *self
                    .definition_constness
                    .get(id)
                    .unwrap_or_else(|| panic!("Missing pureness information for {id}"))
            })
    }

    // Called after all optimizations are done for this `expression`.
    pub(super) fn visit_optimized(&mut self, id: Id, expression: &Expression) {
        let is_pure = self.is_definition_pure(expression);
        self.definition_pureness.insert(id, is_pure);

        let is_const = self.is_definition_const(expression);
        self.definition_constness.insert(id, is_const);

        // TODO: Don't optimize lifted constants again.
        // Then, we can also add asserts here about not visiting them twice.
    }
    pub(super) fn enter_function(&mut self, parameters: &[Id], responsible_parameter: Id) {
        self.definition_pureness
            .extend(parameters.iter().map(|id| (*id, true)));
        let _existing = self.definition_pureness.insert(responsible_parameter, true);
        // TODO: Handle lifted constants properly.
        // assert!(existing.is_none());

        self.definition_constness
            .extend(parameters.iter().map(|id| (*id, false)));
        let _existing = self
            .definition_constness
            .insert(responsible_parameter, false);
        // TODO: Handle lifted constants properly.
        // assert!(existing.is_none());
    }
    pub(super) fn on_normalize_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        fn update(values: &mut FxHashMap<Id, bool>, mapping: &FxHashMap<Id, Id>) {
            *values = values
                .iter()
                .filter_map(|(original_id, value)| {
                    let new_id = mapping.get(original_id)?;
                    Some((*new_id, *value))
                })
                .collect();
        }
        update(&mut self.definition_pureness, mapping);
        update(&mut self.definition_constness, mapping);
    }
    pub(super) fn include(&mut self, other: &Self, mapping: &FxHashMap<Id, Id>) {
        fn insert(
            source: &FxHashMap<Id, bool>,
            mapping: &FxHashMap<Id, Id>,
            target: &mut FxHashMap<Id, bool>,
        ) {
            for (id, source) in source {
                assert!(target.insert(mapping[id], *source).is_none());
            }
        }

        // TODO: Can we avoid some of the cloning?
        insert(
            &other.definition_pureness,
            mapping,
            &mut self.definition_pureness,
        );
        insert(
            &other.definition_constness,
            mapping,
            &mut self.definition_constness,
        );
    }
}

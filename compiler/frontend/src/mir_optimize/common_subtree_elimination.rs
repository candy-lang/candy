//! Common subtree elimination deduplicates pure expressions that yield the same
//! value.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 = builtinIntAdd       |  $0 = builtinIntAdd
//! $1 = 2                   |  $1 = 2
//! $2 = 2                   |  $2 = $1
//! $3 = call $0 with $1 $2  |  $3 = call $0 with $1 $2
//! ```
//!
//! This is especially effective after [constant lifting] because lots of
//! constants are in the same scope. This optimization is also a necessity to
//! avoid exponential code blowup when importing modules – after
//! [module folding], a lot of duplicate functions exist.
//!
//! [constant lifting]: super::constant_lifting
//! [module folding]: super::module_folding

use super::{current_expression::CurrentExpression, pure::PurenessInsights};
use crate::{
    builtin_functions::BuiltinFunction,
    hir,
    id::IdGenerator,
    mir::{Body, Expression, Id, VisitorResult},
    module::Module,
};
use impl_trait_for_tuples::impl_for_tuples;
use itertools::Itertools;
use num_bigint::BigInt;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::{
    collections::hash_map::Entry,
    hash::{Hash, Hasher},
    mem,
};

pub fn eliminate_common_subtrees(body: &mut Body, pureness: &mut PurenessInsights) {
    // Previously, this was a more intuitive `FxHashMap<Id, Expression>`.
    // However, we had to clone _every_ expression for this, which was quite
    // slow.
    //
    // Our new approach uses the `NormalizedComparison` trait defined below.
    // It provides `does_equal_normalized` and `do_hash_normalized` as
    // replacements for `==` and `hash`: Instead of cloning an expression,
    // normalizing all contained IDs, and then calling `==` and `hash` on the
    // normalized expressions, the replacements visit the expression and only
    // track ID replacements via a `NormalizationState`.
    //
    // Due to this additional state parameter, a simple newtype struct as a
    // wrapper doesn't work, and hence we can't use the normal hash map here
    // anymore. Instead, we now map the normalized hash (`u64`) to a list of
    // matches and then check for actual equality in this list.
    //
    // The matches are stored as an index into [body] so that we can read the
    // potentially matching normalized expression and also have mutable access
    // to the current expression within the main loop.
    let mut deterministic_expressions: FxHashMap<u64, Vec<usize>> = FxHashMap::default();

    let mut inner_function_ids: FxHashMap<Id, Vec<Id>> = FxHashMap::default();
    let mut additional_function_hirs: FxHashMap<Id, FxHashSet<hir::Id>> = FxHashMap::default();

    for index in 0..body.expressions.len() {
        let id = body.expressions[index].0;

        let normalized_hash = {
            let expression = &mut body.expressions[index].1;

            if !pureness.is_definition_deterministic(expression) {
                continue;
            }

            if let Expression::Function { body, .. } = &expression {
                inner_function_ids.insert(
                    id,
                    body.all_functions().into_iter().map(|(id, _)| id).collect(),
                );
            }

            expression.do_hash_normalized()
        };

        let existing_entries = deterministic_expressions.entry(normalized_hash);
        match existing_entries {
            Entry::Occupied(mut potential_matches) => {
                let expression = &body.expressions[index].1;
                let Some(canonical_index) = potential_matches
                    .get()
                    .iter()
                    .find(|it| body.expressions[**it].1.does_equal_normalized(expression))
                else {
                    potential_matches.get_mut().push(index);
                    continue;
                };

                let (canonical_id, _) = body.expressions[*canonical_index];

                let mut current_expression = CurrentExpression::new(body, index);
                let old_expression =
                    current_expression.replace_with(Expression::Reference(canonical_id), pureness);

                if let Expression::Function {
                    body,
                    original_hirs,
                    ..
                } = old_expression
                {
                    additional_function_hirs
                        .entry(canonical_id)
                        .or_default()
                        .extend(original_hirs);

                    let canonical_child_functions = inner_function_ids.get(&canonical_id).unwrap();
                    for ((_, child_hirs), canonical_child_id) in body
                        .all_functions()
                        .into_iter()
                        .zip_eq(canonical_child_functions)
                    {
                        additional_function_hirs
                            .entry(*canonical_child_id)
                            .or_default()
                            .extend(child_hirs);
                    }
                }
            }
            Entry::Vacant(_) => {
                existing_entries.insert_entry(vec![index]);
            }
        }
    }

    // Add function HIR IDs to the functions they got normalized into.
    body.visit_mut(&mut |id, expression, _| {
        if let Expression::Function { original_hirs, .. } = expression
                && let Some(additional_hirs) = additional_function_hirs.remove(&id) {
            original_hirs.extend(additional_hirs);
        }
        VisitorResult::Continue
    });
}

impl Body {
    fn all_functions(&self) -> Vec<(Id, FxHashSet<hir::Id>)> {
        let mut ids_and_expressions = vec![];
        self.visit(&mut |id, expression, _| {
            if let Expression::Function { original_hirs, .. } = expression {
                ids_and_expressions.push((id, original_hirs.clone()));
            }
            VisitorResult::Continue
        });
        ids_and_expressions
    }
}

#[derive(Default)]
struct NormalizationState {
    id_generator: IdGenerator<Id>,
    id_mapping: FxHashMap<Id, Id>,
}
impl NormalizationState {
    fn register_body_ids(&mut self, body: &Body) {
        for (id, _) in &body.expressions {
            self.register_defined_id(*id);
        }
    }
    fn register_function_ids(&mut self, parameters: &[Id], responsible_parameter: Id) {
        for parameter in parameters {
            self.register_defined_id(*parameter);
        }
        self.register_defined_id(responsible_parameter);
    }
    fn register_defined_id(&mut self, id: Id) {
        let replacement = self.id_generator.generate();
        assert!(self.id_mapping.insert(id, replacement).is_none());
    }

    fn replacement_for(&mut self, id: Id) -> Id {
        self.id_mapping.get(&id).copied().unwrap_or(id)
    }
}

/// Two functions where local expressions have different IDs are usually not
/// considered equal. This trait calculates normalized hashes expressions by
/// normalizing all locally defined IDs.
trait NormalizedComparison {
    fn does_equal_normalized(&self, other: &Self) -> bool {
        self.equals_normalized(
            &mut NormalizationState::default(),
            other,
            &mut NormalizationState::default(),
        )
    }
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool;

    fn do_hash_normalized(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.hash_normalized(&mut NormalizationState::default(), &mut hasher);
        hasher.finish()
    }
    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher);
}
macro_rules! impl_default_normalized_comparison {
    ($type:ty) => {
        impl NormalizedComparison for $type {
            fn equals_normalized(
                &self,
                _self_normalization: &mut NormalizationState,
                other: &Self,
                _other_normalization: &mut NormalizationState,
            ) -> bool {
                self == other
            }

            fn hash_normalized(
                &self,
                _normalization: &mut NormalizationState,
                state: &mut impl Hasher,
            ) {
                self.hash(state);
            }
        }
    };
    ($($type:ty),*) => {
        $(impl_default_normalized_comparison!($type);)*
    };
}
impl_default_normalized_comparison!(BigInt, BuiltinFunction, hir::Id, Module, String, usize);
impl<T: NormalizedComparison> NormalizedComparison for Option<T> {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(self_value), Some(other_value)) => {
                self_value.equals_normalized(self_normalization, other_value, other_normalization)
            }
            _ => false,
        }
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        mem::discriminant(self).hash(state);
        if let Some(value) = self {
            value.hash_normalized(normalization, state);
        }
    }
}
impl<T: NormalizedComparison> NormalizedComparison for [T] {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .zip_eq(other.iter())
            .all(|(self_item, other_item)| {
                self_item.equals_normalized(self_normalization, other_item, other_normalization)
            })
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        state.write_length_prefix(self.len());
        for item in self {
            item.hash_normalized(normalization, state);
        }
    }
}
#[impl_for_tuples(1, 2)]
impl NormalizedComparison for Tuple {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        for_tuples!( #(self.Tuple.equals_normalized(self_normalization, &other.Tuple, other_normalization))&* )
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        for_tuples!( #(Tuple.hash_normalized(normalization, state);)*  );
    }
}
impl NormalizedComparison for Id {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        self_normalization.replacement_for(*self) == other_normalization.replacement_for(*other)
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        normalization.replacement_for(*self).hash(state);
    }
}
impl NormalizedComparison for Body {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        self_normalization.register_body_ids(self);
        other_normalization.register_body_ids(other);

        self.expressions.equals_normalized(
            self_normalization,
            &other.expressions,
            other_normalization,
        )
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        normalization.register_body_ids(self);

        self.expressions.hash_normalized(normalization, state);
    }
}
// Only `Expression::Function` is handled specially, the remaining cases just
// forward calls to their fields.
impl NormalizedComparison for Expression {
    fn equals_normalized(
        &self,
        self_normalization: &mut NormalizationState,
        other: &Self,
        other_normalization: &mut NormalizationState,
    ) -> bool {
        match (self, other) {
            (Self::Int(self_int), Self::Int(other_int)) => {
                self_int.equals_normalized(self_normalization, other_int, other_normalization)
            }
            (Self::Text(self_text), Self::Text(other_text)) => {
                self_text.equals_normalized(self_normalization, other_text, other_normalization)
            }
            (
                Self::Tag {
                    symbol: self_symbol,
                    value: self_value,
                },
                Self::Tag {
                    symbol: other_symbol,
                    value: other_value,
                },
            ) => {
                self_symbol.equals_normalized(self_normalization, other_symbol, other_normalization)
                    && self_value.equals_normalized(
                        self_normalization,
                        other_value,
                        other_normalization,
                    )
            }
            (Self::Builtin(self_builtin), Self::Builtin(other_builtin)) => self_builtin
                .equals_normalized(self_normalization, other_builtin, other_normalization),
            (Self::List(self_items), Self::List(other_items)) => {
                self_items.equals_normalized(self_normalization, other_items, other_normalization)
            }
            (Self::Struct(self_fields), Self::Struct(other_fields)) => {
                self_fields.equals_normalized(self_normalization, other_fields, other_normalization)
            }
            (Self::Reference(self_id), Self::Reference(other_id)) => {
                self_id.equals_normalized(self_normalization, other_id, other_normalization)
            }
            (Self::HirId(self_id), Self::HirId(other_id)) => {
                self_id.equals_normalized(self_normalization, other_id, other_normalization)
            }
            (
                Self::Function {
                    original_hirs: _,
                    parameters: self_parameters,
                    responsible_parameter: self_responsible_parameter,
                    body: self_body,
                },
                Self::Function {
                    original_hirs: _,
                    parameters: other_parameters,
                    responsible_parameter: other_responsible_parameter,
                    body: other_body,
                },
            ) => {
                self_normalization
                    .register_function_ids(self_parameters, *self_responsible_parameter);
                other_normalization
                    .register_function_ids(other_parameters, *other_responsible_parameter);

                self_parameters.equals_normalized(
                    self_normalization,
                    other_parameters,
                    other_normalization,
                ) && self_responsible_parameter.equals_normalized(
                    self_normalization,
                    other_responsible_parameter,
                    other_normalization,
                ) && self_body.equals_normalized(
                    self_normalization,
                    other_body,
                    other_normalization,
                )
            }
            (Self::Parameter, Self::Parameter) => true,
            (
                Self::Call {
                    function: self_function,
                    arguments: self_arguments,
                    responsible: self_responsible,
                },
                Self::Call {
                    function: other_function,
                    arguments: other_arguments,
                    responsible: other_responsible,
                },
            ) => {
                self_function.equals_normalized(
                    self_normalization,
                    other_function,
                    other_normalization,
                ) && self_arguments.equals_normalized(
                    self_normalization,
                    other_arguments,
                    other_normalization,
                ) && self_responsible.equals_normalized(
                    self_normalization,
                    other_responsible,
                    other_normalization,
                )
            }
            (
                Self::UseModule {
                    current_module: self_current_module,
                    relative_path: self_relative_path,
                    responsible: self_responsible,
                },
                Self::UseModule {
                    current_module: other_current_module,
                    relative_path: other_relative_path,
                    responsible: other_responsible,
                },
            ) => {
                self_current_module.equals_normalized(
                    self_normalization,
                    other_current_module,
                    other_normalization,
                ) && self_relative_path.equals_normalized(
                    self_normalization,
                    other_relative_path,
                    other_normalization,
                ) && self_responsible.equals_normalized(
                    self_normalization,
                    other_responsible,
                    other_normalization,
                )
            }
            (
                Self::Panic {
                    reason: self_reason,
                    responsible: self_responsible,
                },
                Self::Panic {
                    reason: other_reason,
                    responsible: other_responsible,
                },
            ) => {
                self_reason.equals_normalized(self_normalization, other_reason, other_normalization)
                    && self_responsible.equals_normalized(
                        self_normalization,
                        other_responsible,
                        other_normalization,
                    )
            }
            (
                Self::TraceCallStarts {
                    hir_call: self_hir_call,
                    function: self_function,
                    arguments: self_arguments,
                    responsible: self_responsible,
                },
                Self::TraceCallStarts {
                    hir_call: other_hir_call,
                    function: other_function,
                    arguments: other_arguments,
                    responsible: other_responsible,
                },
            ) => {
                self_hir_call.equals_normalized(
                    self_normalization,
                    other_hir_call,
                    other_normalization,
                ) && self_function.equals_normalized(
                    self_normalization,
                    other_function,
                    other_normalization,
                ) && self_arguments.equals_normalized(
                    self_normalization,
                    other_arguments,
                    other_normalization,
                ) && self_responsible.equals_normalized(
                    self_normalization,
                    other_responsible,
                    other_normalization,
                )
            }
            (
                Self::TraceCallEnds {
                    return_value: self_return_value,
                },
                Self::TraceCallEnds {
                    return_value: other_return_value,
                },
            ) => self_return_value.equals_normalized(
                self_normalization,
                other_return_value,
                other_normalization,
            ),
            (
                Self::TraceExpressionEvaluated {
                    hir_expression: self_hir_expression,
                    value: self_value,
                },
                Self::TraceExpressionEvaluated {
                    hir_expression: other_hir_expression,
                    value: other_value,
                },
            ) => {
                self_hir_expression.equals_normalized(
                    self_normalization,
                    other_hir_expression,
                    other_normalization,
                ) && self_value.equals_normalized(
                    self_normalization,
                    other_value,
                    other_normalization,
                )
            }
            (
                Self::TraceFoundFuzzableFunction {
                    hir_definition: self_hir_definition,
                    function: self_function,
                },
                Self::TraceFoundFuzzableFunction {
                    hir_definition: other_hir_definition,
                    function: other_function,
                },
            ) => {
                self_hir_definition.equals_normalized(
                    self_normalization,
                    other_hir_definition,
                    other_normalization,
                ) && self_function.equals_normalized(
                    self_normalization,
                    other_function,
                    other_normalization,
                )
            }
            _ => false,
        }
    }

    fn hash_normalized(&self, normalization: &mut NormalizationState, state: &mut impl Hasher) {
        mem::discriminant(self).hash(state);
        match self {
            Self::Int(int) => int.hash_normalized(normalization, state),
            Self::Text(text) => text.hash_normalized(normalization, state),
            Self::Tag { symbol, value } => {
                symbol.hash_normalized(normalization, state);
                value.hash_normalized(normalization, state);
            }
            Self::Builtin(builtin) => builtin.hash_normalized(normalization, state),
            Self::List(items) => items.hash_normalized(normalization, state),
            Self::Struct(fields) => fields.len().hash_normalized(normalization, state),
            Self::Reference(id) => id.hash_normalized(normalization, state),
            Self::HirId(id) => id.hash_normalized(normalization, state),
            Self::Function {
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
            } => {
                normalization.register_function_ids(parameters, *responsible_parameter);

                parameters.hash_normalized(normalization, state);
                responsible_parameter.hash_normalized(normalization, state);
                body.hash_normalized(normalization, state);
            }
            Self::Parameter => {}
            Self::Call {
                function,
                arguments,
                responsible,
            } => {
                function.hash_normalized(normalization, state);
                arguments.hash_normalized(normalization, state);
                responsible.hash_normalized(normalization, state);
            }
            Self::UseModule {
                current_module,
                relative_path,
                responsible,
            } => {
                current_module.hash_normalized(normalization, state);
                relative_path.hash_normalized(normalization, state);
                responsible.hash_normalized(normalization, state);
            }
            Self::Panic {
                reason,
                responsible,
            } => {
                reason.hash_normalized(normalization, state);
                responsible.hash_normalized(normalization, state);
            }
            Self::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                hir_call.hash_normalized(normalization, state);
                function.hash_normalized(normalization, state);
                arguments.hash_normalized(normalization, state);
                responsible.hash_normalized(normalization, state);
            }
            Self::TraceCallEnds { return_value } => {
                return_value.hash_normalized(normalization, state);
            }
            Self::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                hir_expression.hash_normalized(normalization, state);
                value.hash_normalized(normalization, state);
            }
            Self::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                hir_definition.hash_normalized(normalization, state);
                function.hash_normalized(normalization, state);
            }
        }
    }
}

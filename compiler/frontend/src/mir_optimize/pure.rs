//! Some optimizations rely on some properties of expressions.
//!
//! Deterministic < Pure < Const
//!
//! # Deterministic
//!
//! Running the expression twice gives the same result.
//!
//! Examples for deterministic expressions:
//!
//! - `4`
//! - `call needs with False "blub" $3` (this will always panic in the same way)
//!
//! # Pure
//!
//! Running the expression twice gives the same result _and_ it can be removed
//! if it's not referenced.
//!
//! Referenced expressions can still be impure. For example, each struct
//! defintion is pure, although it may contain impure values such as channels.
//! Function defintions are also pure.
//!
//! # Const
//!
//! Const expressions are compile-time known. All captured expressions must also
//! be compile-time known.

use crate::{
    builtin_functions::BuiltinFunction,
    mir::{Expression, Id},
};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PurenessInsights {
    pure_definitions: FxHashSet<Id>,
    pure_functions: FxHashSet<Id>,
    const_definitions: FxHashSet<Id>,
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
            | Expression::Builtin(_)
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::Reference(_)
            | Expression::HirId(_)
            | Expression::Function { .. }
            | Expression::Parameter => true,
            // TODO: Check whether executing the function with the given arguments is pure when we inspect data flow.
            Expression::Call { function, .. } => self.pure_functions.contains(function),
            Expression::UseModule { .. } | Expression::Panic { .. } => false,
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }
    pub fn is_function_pure(&self, expression: &Expression) -> bool {
        match expression {
            Expression::Builtin(builtin) => match builtin {
                BuiltinFunction::Equals
                | BuiltinFunction::GetArgumentCount
                | BuiltinFunction::IntAdd
                | BuiltinFunction::IntBitLength
                | BuiltinFunction::IntBitwiseAnd
                | BuiltinFunction::IntBitwiseOr
                | BuiltinFunction::IntBitwiseXor
                | BuiltinFunction::IntCompareTo
                | BuiltinFunction::IntDivideTruncating
                | BuiltinFunction::IntModulo
                | BuiltinFunction::IntMultiply
                | BuiltinFunction::IntParse
                | BuiltinFunction::IntRemainder
                | BuiltinFunction::IntShiftLeft
                | BuiltinFunction::IntShiftRight
                | BuiltinFunction::IntSubtract
                | BuiltinFunction::ListFilled
                | BuiltinFunction::ListGet
                | BuiltinFunction::ListInsert
                | BuiltinFunction::ListLength
                | BuiltinFunction::ListRemoveAt
                | BuiltinFunction::ListReplace
                | BuiltinFunction::StructGet
                | BuiltinFunction::StructGetKeys
                | BuiltinFunction::StructHasKey
                | BuiltinFunction::TagGetValue
                | BuiltinFunction::TagHasValue
                | BuiltinFunction::TagWithoutValue
                | BuiltinFunction::TextCharacters
                | BuiltinFunction::TextConcatenate
                | BuiltinFunction::TextContains
                | BuiltinFunction::TextEndsWith
                | BuiltinFunction::TextFromUtf8
                | BuiltinFunction::TextGetRange
                | BuiltinFunction::TextIsEmpty
                | BuiltinFunction::TextLength
                | BuiltinFunction::TextStartsWith
                | BuiltinFunction::TextTrimEnd
                | BuiltinFunction::TextTrimStart
                | BuiltinFunction::ToDebugText
                | BuiltinFunction::TypeOf => true,
                _ => false,
            },
            Expression::Function {
                original_hirs,
                parameters,
                responsible_parameter,
                body,
            } => body
                .iter()
                .all(|(_, expression)| self.is_definition_pure(expression)),
            _ => false,
        }
    }
    pub fn is_definition_const(&self, expression: &Expression) -> bool {
        self.is_definition_pure(expression)
            && expression
                .captured_ids()
                .iter()
                .all(|id| self.const_definitions.contains(id))
    }

    // Called after all optimizations are done for this `expression`.
    pub(super) fn visit_optimized(&mut self, id: Id, expression: &Expression) {
        if self.is_definition_pure(expression) {
            self.pure_definitions.insert(id);
        }

        if self.is_definition_const(expression) {
            self.const_definitions.insert(id);
        }

        // TODO: Don't optimize lifted constants again.
        // Then, we can also add asserts here about not visiting them twice.
    }
    pub(super) fn enter_function(&mut self, parameters: &[Id], responsible_parameter: Id) {
        self.pure_definitions.extend(parameters.iter().copied());
        let _existing = self.pure_definitions.insert(responsible_parameter);
        // TODO: Handle lifted constants properly.
        // assert!(existing.is_none());

        // TODO: Handle lifted constants properly.
        // assert!(existing.is_none());
    }
    pub(super) fn on_normalize_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        fn update(values: &mut FxHashSet<Id>, mapping: &FxHashMap<Id, Id>) {
            *values = values
                .iter()
                .filter_map(|original_id| {
                    let new_id = mapping.get(original_id)?;
                    Some(*new_id)
                })
                .collect();
        }
        update(&mut self.pure_definitions, mapping);
        update(&mut self.const_definitions, mapping);
    }
    pub(super) fn include(&mut self, other: &Self, mapping: &FxHashMap<Id, Id>) {
        fn insert(source: &FxHashSet<Id>, mapping: &FxHashMap<Id, Id>, target: &mut FxHashSet<Id>) {
            for id in source {
                assert!(target.insert(mapping[id]));
            }
        }

        // TODO: Can we avoid some of the cloning?
        insert(&other.pure_definitions, mapping, &mut self.pure_definitions);
        insert(
            &other.const_definitions,
            mapping,
            &mut self.const_definitions,
        );
    }
}

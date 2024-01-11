//! Some optimizations rely on some properties of expressions.
//!
//! Deterministic < Pure < Const
//!
//! # Deterministic
//!
//! Running the expression twice gives the same result. If this expression
//! occurs multiple times in a body, only one of them needs to be kept. The
//! usages of the removed expression can reference the kept expression instead.
//!
//! Deterministic examples:
//!
//! - `4`
//! - `call needs with False $0 $1` (this will always panic in the same way)
//!
//! Non-deterministic examples:
//!
//! - `channel.send 5` (has side effects besides panicking)
//! - `use $0` (a module might create a channel globally and export it)
//!
//! # Pure
//!
//! Running the expression twice gives the same result _and_ it has no side
//! effects, including panicking. If the expression is not referenced by any
//! other code, it can safely be removed.
//!
//! Pure examples:
//!
//! - `4`
//! - `call ✨.intAdd $0 $1` (this can never panic)
//! - `[$0: $1]` (even if $0 and $1 are impure)
//! - `{ channel.send $0 $1 }` (only _running_ the function is impure)
//!
//! Impure examples:
//!
//! - `call builtins.intAdd $0 $1` (this has needs and can panic)
//!
//! # Const
//!
//! Const expressions are compile-time known. All captured expressions must also
//! be compile-time known.

use crate::{
    builtin_functions::BuiltinFunction,
    mir::{Expression, Id},
    utils::HashSetExtension,
};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PurenessInsights {
    deterministic_definitions: FxHashSet<Id>,
    deterministic_functions: FxHashSet<Id>,
    pure_definitions: FxHashSet<Id>,
    pure_functions: FxHashSet<Id>,
    const_definitions: FxHashSet<Id>,
}
impl PurenessInsights {
    pub fn is_definition_deterministic(&self, expression: &Expression) -> bool {
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
            | Expression::Parameter
            | Expression::Panic { .. } => true,
            Expression::Call { function, .. } => self.deterministic_functions.contains(function),
            Expression::UseModule { .. }
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }
    pub fn is_function_deterministic(&self, expression: &Expression) -> bool {
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
                BuiltinFunction::FunctionRun | BuiltinFunction::IfElse | BuiltinFunction::Print => {
                    false
                }
            },
            Expression::Function { body, .. } => body
                .iter()
                .all(|(_, expression)| self.is_definition_deterministic(expression)),
            Expression::Tag { .. } => true, // either works or panics
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::HirId(_)
            | Expression::UseModule { .. } => true, // always panics
            Expression::Parameter
            | Expression::Call { .. }
            | Expression::Panic { .. }
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. }
            | Expression::Reference(_) => false,
        }
    }

    pub fn is_definition_pure(&self, expression: &Expression) -> bool {
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
            Expression::Call { function, .. } => self.pure_functions.contains(function),
            Expression::UseModule { .. } | Expression::Panic { .. } => false,
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceTailCall { .. }
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
                BuiltinFunction::FunctionRun | BuiltinFunction::IfElse | BuiltinFunction::Print => {
                    false
                }
            },
            Expression::Function { body, .. } => body
                .iter()
                .all(|(_, expression)| self.is_definition_pure(expression)),
            Expression::Tag { value: None, .. } => true,
            _ => false, // calling anything else will panic
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
        if self.is_definition_deterministic(expression) {
            self.deterministic_definitions.insert(id);
        }
        if self.is_function_deterministic(expression) {
            self.deterministic_functions.insert(id);
        }

        if self.is_definition_pure(expression) {
            self.pure_definitions.insert(id);
        }
        if self.is_function_pure(expression) {
            self.pure_functions.insert(id);
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
        update(&mut self.deterministic_definitions, mapping);
        update(&mut self.deterministic_functions, mapping);
        update(&mut self.pure_definitions, mapping);
        update(&mut self.pure_functions, mapping);
        update(&mut self.const_definitions, mapping);
    }
    pub(super) fn on_remove(&mut self, id: Id) {
        let Self {
            deterministic_definitions,
            deterministic_functions,
            pure_definitions,
            pure_functions,
            const_definitions,
        } = self;
        deterministic_definitions.remove(&id);
        deterministic_functions.remove(&id);
        pure_definitions.remove(&id);
        pure_functions.remove(&id);
        const_definitions.remove(&id);
    }
    pub(super) fn include(&mut self, other: &Self, mapping: &FxHashMap<Id, Id>) {
        fn insert(source: &FxHashSet<Id>, mapping: &FxHashMap<Id, Id>, target: &mut FxHashSet<Id>) {
            for id in source {
                let replacement = *mapping
                    .get(id)
                    .unwrap_or_else(|| panic!("Missing mapping for {id}"));
                target.force_insert(replacement);
            }
        }

        // TODO: Can we avoid some of the cloning?
        insert(
            &other.deterministic_definitions,
            mapping,
            &mut self.deterministic_definitions,
        );
        insert(
            &other.deterministic_functions,
            mapping,
            &mut self.deterministic_functions,
        );
        insert(&other.pure_definitions, mapping, &mut self.pure_definitions);
        insert(&other.pure_functions, mapping, &mut self.pure_functions);
        insert(
            &other.const_definitions,
            mapping,
            &mut self.const_definitions,
        );
    }
}

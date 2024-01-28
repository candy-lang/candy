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
    id::CountableId,
    mir::{Expression, Id},
};
use bitvec::vec::BitVec;
use rustc_hash::FxHashMap;
use std::iter;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PurenessInsights {
    deterministic_definitions: IdSet,
    deterministic_functions: IdSet,
    pure_definitions: IdSet,
    pure_functions: IdSet,
    const_definitions: IdSet,
}
impl PurenessInsights {
    #[must_use]
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
            Expression::Call { function, .. } => self.deterministic_functions.contains(*function),
            Expression::UseModule { .. }
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceTailCall { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }
    #[must_use]
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
            | Expression::UseModule { .. }
            | Expression::Panic { .. } => true, // always panics
            Expression::Parameter
            | Expression::Call { .. }
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceTailCall { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. }
            | Expression::Reference(_) => false,
        }
    }

    #[must_use]
    pub const fn pure_definitions(&self) -> &IdSet {
        &self.pure_definitions
    }
    #[must_use]
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
            Expression::Call { function, .. } => self.pure_functions.contains(*function),
            Expression::UseModule { .. } | Expression::Panic { .. } => false,
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceTailCall { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }

    #[must_use]
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
    #[must_use]
    pub fn is_definition_const(&self, expression: &Expression) -> bool {
        self.is_definition_pure(expression)
            && expression
                .captured_ids()
                .iter()
                .all(|id| self.const_definitions.contains(*id))
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
        self.pure_definitions.extend(parameters);
        self.pure_definitions.insert(responsible_parameter);
        // TODO: Handle lifted constants properly.
        // assert!(existing.is_none());
    }
    pub(super) fn on_normalize_ids(&mut self, mapping: &FxHashMap<Id, Id>) {
        fn update(values: &mut IdSet, mapping: &FxHashMap<Id, Id>) {
            *values = values
                .iter()
                .filter_map(|original_id| {
                    let new_id = mapping.get(&original_id)?;
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
        deterministic_definitions.remove(id);
        deterministic_functions.remove(id);
        pure_definitions.remove(id);
        pure_functions.remove(id);
        const_definitions.remove(id);
    }
    pub(super) fn include(&mut self, other: &Self, mapping: &FxHashMap<Id, Id>) {
        fn insert(source: &IdSet, mapping: &FxHashMap<Id, Id>, target: &mut IdSet) {
            for id in source {
                let replacement = *mapping
                    .get(&id)
                    .unwrap_or_else(|| panic!("Missing mapping for {id}"));
                target.insert(replacement);
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

/// This behaves like a [`HashSet<Id>`], but it's more efficient for our use
/// case: We store a [`BitVec`] where each index corresponds to an [`Id`]
/// because our [`Id`]s are numbered sequentially.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IdSet(BitVec);
impl IdSet {
    #[must_use]
    pub fn contains(&self, id: Id) -> bool {
        if id.to_usize() >= self.0.len() {
            false
        } else {
            self.0[id.to_usize()]
        }
    }

    #[must_use]
    pub fn iter(&self) -> IdSetIter {
        self.into_iter()
    }

    pub fn insert(&mut self, id: Id) {
        let additional_length_to_reserve = (id.to_usize() + 1).saturating_sub(self.0.len());
        if additional_length_to_reserve > 0 {
            self.0
                .extend(iter::repeat(false).take(additional_length_to_reserve));
        }
        self.0.set(id.to_usize(), true);
    }
    pub fn remove(&mut self, id: Id) {
        if id.to_usize() >= self.0.len() {
            return;
        }
        self.0.set(id.to_usize(), false);
    }
}

impl<'a> IntoIterator for &'a IdSet {
    type IntoIter = IdSetIter<'a>;
    type Item = Id;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IdSetIter {
            values: self,
            index: 0,
        }
    }
}
pub struct IdSetIter<'a> {
    values: &'a IdSet,
    index: usize,
}
impl<'a> Iterator for IdSetIter<'a> {
    type Item = Id;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.values.0.len() {
            return None;
        }

        loop {
            self.index += 1;
            if self.index >= self.values.0.len() {
                return None;
            }
            if self.values.0[self.index] {
                return Some(Id::from_usize(self.index));
            }
        }
    }
}

impl FromIterator<Id> for IdSet {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Id>,
    {
        let mut result = Self::default();
        for id in iter {
            result.insert(id);
        }
        result
    }
}

impl<'a> Extend<&'a Id> for IdSet {
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = &'a Id>,
    {
        for id in iter {
            self.insert(*id);
        }
    }
}

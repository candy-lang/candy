//! Constant folding is just a fancy term for executing instructions at
//! compile-time when their result is known.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 = builtinIntAdd       |  $0 = builtinIntAdd
//! $1 = 2                   |  $1 = 2
//! $2 = call $0 with $1 $1  |  $2 = 4
//! ```
//!
//! Afterwards, [tree shaking] can remove unneeded arguments. In the example
//! above, only `$2` would remain.
//!
//! Not all arguments need to be compile-time known. For example, even this code
//! could be simplified:
//!
//! ```mir
//! $0 = Foo                 |  $0 = Foo
//! $1 = struct [$0: $a]     |  $1 = struct [$0: $a]
//! $2 = builtinStructGet    |  $2 = builtinStructGet
//! $3 = call $3 with $1 $0  |  $3 = $a
//! ```
//!
//! Not only builtins can be compile-time evaluated: Needs and compile-time
//! errors from previous compilation stages can possibly also be executed at
//! compile-time.
//!
//! [tree shaking]: super::tree_shaking

use super::{
    current_expression::{Context, CurrentExpression},
    pure::PurenessInsights,
};
use crate::{
    builtin_functions::BuiltinFunction,
    format::{format_value, FormatValue, MaxLength, Precedence},
    id::IdGenerator,
    mir::{Body, Expression, Id, VisibleExpressions},
};
use itertools::Itertools;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};
use std::{
    borrow::Cow,
    cmp::Ordering,
    str::{self, FromStr},
};
use tracing::warn;
use unicode_segmentation::UnicodeSegmentation;

pub fn fold_constants(context: &mut Context, expression: &mut CurrentExpression) {
    let Expression::Call {
        function,
        arguments,
        responsible,
    } = &**expression
    else {
        return;
    };

    match context.visible.get(*function) {
        Expression::Tag {
            symbol,
            value: None,
        } if arguments.len() == 1 => {
            expression.replace_with(
                Expression::Tag {
                    symbol: symbol.clone(),
                    value: Some(arguments[0]),
                },
                context.pureness,
            );
        }
        Expression::Builtin(builtin) => {
            let arguments = arguments.clone();
            let responsible = *responsible;
            let Some(result) = run_builtin(
                &mut *expression,
                *builtin,
                &arguments,
                responsible,
                context.visible,
                context.id_generator,
                context.pureness,
            ) else {
                return;
            };
            expression.replace_with(result, context.pureness);
        }
        _ => {}
    }
}
/// This function tries to run a builtin, requiring a minimal amount of static
/// knowledge. For example, it can find out that the result of `✨.equals $3 $3`
/// is `True`, even if the value of `$3` is not known at compile-time.
///
/// Returns `None` if the call couldn't be evaluated statically.
fn run_builtin(
    expression: &mut CurrentExpression,
    builtin: BuiltinFunction,
    arguments: &[Id],
    responsible: Id,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
    pureness: &mut PurenessInsights,
) -> Option<Expression> {
    debug_assert_eq!(
        arguments.len(),
        builtin.num_parameters(),
        "Wrong number of arguments for calling {builtin}",
    );

    let result = match builtin {
        BuiltinFunction::Equals => {
            let [a, b] = arguments else { unreachable!() };
            a.semantically_equals(*b, visible, pureness)?.into()
        }
        BuiltinFunction::FunctionRun => {
            let [function] = arguments else {
                unreachable!()
            };
            Expression::Call {
                function: *function,
                arguments: vec![],
                responsible,
            }
        }
        BuiltinFunction::GetArgumentCount => {
            let [function] = arguments else {
                unreachable!()
            };
            match visible.get(*function) {
                Expression::Builtin(builtin) => builtin.num_parameters().into(),
                Expression::Function { parameters, .. } => parameters.len().into(),
                _ => return None,
            }
        }
        BuiltinFunction::IfElse => {
            let [condition, then, else_] = arguments else {
                unreachable!()
            };
            if let Ok(condition) = visible.get(*condition).try_into() {
                // if true foo bar -> foo
                // if false foo bar -> bar
                Expression::Call {
                    function: if condition { *then } else { *else_ },
                    arguments: vec![],
                    responsible,
                }
            } else {
                // if foo { True } { False } -> foo
                match (visible.get(*then), visible.get(*else_)) {
                    (
                        Expression::Function { body: a, .. },
                        Expression::Function { body: b, .. },
                    ) => match (&a.expressions[..], &b.expressions[..]) {
                        ([(_, a)], [(_, b)]) => {
                            if a.try_into().ok()? && !b.try_into().ok()? {
                                Expression::Reference(*condition)
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }
        BuiltinFunction::IntAdd => {
            let [a, b] = arguments else { unreachable!() };
            let a: &BigInt = visible.get(*a).try_into().ok()?;
            let b: &BigInt = visible.get(*b).try_into().ok()?;
            (a + b).into()
        }
        BuiltinFunction::IntBitLength => {
            let [a] = arguments else { unreachable!() };
            let a: &BigInt = visible.get(*a).try_into().ok()?;
            a.bits().into()
        }
        BuiltinFunction::IntBitwiseAnd => {
            let [a, b] = arguments else { unreachable!() };
            if a.semantically_equals(*b, visible, pureness) == Some(true) {
                return Some(Expression::Reference(*a));
            }

            let a: &BigInt = visible.get(*a).try_into().ok()?;
            let b: &BigInt = visible.get(*b).try_into().ok()?;
            (a & b).into()
        }
        BuiltinFunction::IntBitwiseOr => {
            let [a, b] = arguments else { unreachable!() };
            if a.semantically_equals(*b, visible, pureness) == Some(true) {
                return Some(Expression::Reference(*a));
            }

            let a: &BigInt = visible.get(*a).try_into().ok()?;
            let b: &BigInt = visible.get(*b).try_into().ok()?;
            (a | b).into()
        }
        BuiltinFunction::IntBitwiseXor => {
            let [a, b] = arguments else { unreachable!() };
            if a.semantically_equals(*b, visible, pureness) == Some(true) {
                return Some(0.into());
            }

            let a: &BigInt = visible.get(*a).try_into().ok()?;
            let b: &BigInt = visible.get(*b).try_into().ok()?;
            (a ^ b).into()
        }
        BuiltinFunction::IntCompareTo => {
            let [a, b] = arguments else { unreachable!() };
            if a.semantically_equals(*b, visible, pureness) == Some(true) {
                return Some(Ordering::Equal.into());
            }

            let a: &BigInt = visible.get(*a).try_into().ok()?;
            let b: &BigInt = visible.get(*b).try_into().ok()?;
            a.cmp(b).into()
        }
        BuiltinFunction::IntDivideTruncating => {
            let [dividend, divisor] = arguments else {
                unreachable!()
            };
            if dividend.semantically_equals(*divisor, visible, pureness) == Some(true) {
                return Some(1.into());
            }

            let dividend: &BigInt = visible.get(*dividend).try_into().ok()?;
            let divisor: &BigInt = visible.get(*divisor).try_into().ok()?;
            (dividend / divisor).into()
        }
        BuiltinFunction::IntModulo => {
            let [dividend, divisor] = arguments else {
                unreachable!()
            };
            if dividend.semantically_equals(*divisor, visible, pureness) == Some(true) {
                return Some(0.into());
            }

            let dividend: &BigInt = visible.get(*dividend).try_into().ok()?;
            let divisor: &BigInt = visible.get(*divisor).try_into().ok()?;
            dividend.mod_floor(divisor).into()
        }
        BuiltinFunction::IntMultiply => {
            let [factor_a, factor_b] = arguments else {
                unreachable!()
            };
            let factor_a: &BigInt = visible.get(*factor_a).try_into().ok()?;
            let factor_b: &BigInt = visible.get(*factor_b).try_into().ok()?;
            (factor_a * factor_b).into()
        }
        BuiltinFunction::IntParse => {
            let [text] = arguments else { unreachable!() };
            let text: &str = visible.get(*text).try_into().ok()?;
            let mut body = Body::default();
            let result = match BigInt::from_str(text) {
                Ok(value) => Ok(body.push_with_new_id(id_generator, value)),
                Err(err) => Err(body.push_with_new_id(id_generator, err.to_string())),
            };
            body.push_with_new_id(id_generator, result);
            expression.replace_with_multiple(body, pureness);
            return None;
        }
        BuiltinFunction::IntRemainder => {
            let [dividend, divisor] = arguments else {
                unreachable!()
            };
            if dividend.semantically_equals(*divisor, visible, pureness) == Some(true) {
                return Some(0.into());
            }

            let dividend: &BigInt = visible.get(*dividend).try_into().ok()?;
            let divisor: &BigInt = visible.get(*divisor).try_into().ok()?;
            (dividend % divisor).into()
        }
        BuiltinFunction::IntShiftLeft => {
            let [value, amount] = arguments else {
                unreachable!()
            };
            let amount: &BigInt = visible.get(*amount).try_into().ok()?;
            // TODO: Support larger shift amounts.
            let amount: u128 = amount.try_into().unwrap();
            if amount == 0 {
                return Some(value.into());
            }

            let value: &BigInt = visible.get(*value).try_into().ok()?;
            (value << amount).into()
        }
        BuiltinFunction::IntShiftRight => {
            let [value, amount] = arguments else {
                unreachable!()
            };
            let amount: &BigInt = visible.get(*amount).try_into().ok()?;
            // TODO: Support larger shift amounts.
            let amount: u128 = amount.try_into().unwrap();
            if amount == 0 {
                return Some(value.into());
            }

            let value: &BigInt = visible.get(*value).try_into().ok()?;
            (value >> amount).into()
        }
        BuiltinFunction::IntSubtract => {
            let [minuend, subtrahend] = arguments else {
                unreachable!()
            };
            if minuend.semantically_equals(*subtrahend, visible, pureness) == Some(true) {
                return Some(Expression::Int(0.into()));
            }

            let minuend: &BigInt = visible.get(*minuend).try_into().ok()?;
            let subtrahend: &BigInt = visible.get(*subtrahend).try_into().ok()?;
            (minuend - subtrahend).into()
        }
        BuiltinFunction::ListFilled => {
            let [length, item] = arguments else {
                unreachable!()
            };
            let Expression::Int(length) = visible.get(*length) else {
                return None;
            };
            // TODO: Support lists longer than `usize::MAX`.
            vec![*item; length.to_usize().unwrap()].into()
        }
        BuiltinFunction::ListGet => {
            let [list, index] = arguments else {
                unreachable!()
            };
            let Expression::List(list) = visible.get(*list) else {
                return None;
            };
            let Expression::Int(index) = visible.get(*index) else {
                return None;
            };
            // TODO: Support lists longer than `usize::MAX`.
            list.get(index.to_usize().unwrap())?.into()
        }
        BuiltinFunction::ListInsert => return None,
        BuiltinFunction::ListLength => {
            let [list] = arguments else { unreachable!() };
            let Expression::List(list) = visible.get(*list) else {
                return None;
            };
            list.len().into()
        }
        BuiltinFunction::ListRemoveAt => return None,
        BuiltinFunction::ListReplace => return None,
        BuiltinFunction::Print => return None,
        BuiltinFunction::StructGet => {
            let [struct_, key] = arguments else {
                unreachable!()
            };
            let Expression::Struct(fields) = visible.get(*struct_) else {
                return None;
            };

            // TODO: Relax this requirement. Even if not all keys are
            // constant, we may still conclude the result of the builtin:
            // If one key `semantically_equals` the requested one and all
            // others definitely not, then we can still resolve that.
            if !pureness.is_definition_const(visible.get(*key)) {
                return None;
            }
            if fields
                .iter()
                .any(|(id, _)| !pureness.is_definition_const(visible.get(*id)))
            {
                return None;
            }

            let value = fields
                .iter()
                .rev()
                .find(|(k, _)| {
                    k.semantically_equals(*key, visible, pureness)
                        .unwrap_or_default()
                })
                .map(|(_, value)| *value);
            if let Some(value) = value {
                Expression::Reference(value)
            } else {
                warn!(
                    "Struct access will panic because key {} isn't in there.",
                    visible.get(*key),
                );
                return None;
            }
        }
        BuiltinFunction::StructGetKeys => return None,
        BuiltinFunction::StructHasKey => {
            let [struct_, key] = arguments else {
                unreachable!()
            };
            let Expression::Struct(fields) = visible.get(*struct_) else {
                return None;
            };

            let mut is_contained = Some(false);
            for (k, _) in fields {
                match k.semantically_equals(*key, visible, pureness) {
                    Some(is_equal) => {
                        if is_equal {
                            is_contained = Some(true);
                            break;
                        }
                    }
                    None => {
                        if is_contained == Some(false) {
                            is_contained = None;
                        }
                    }
                }
            }

            is_contained?.into()
        }
        BuiltinFunction::TagGetValue => {
            let [tag] = arguments else { unreachable!() };
            let Expression::Tag {
                value: Some(value), ..
            } = visible.get(*tag)
            else {
                return None;
            };
            value.into()
        }
        BuiltinFunction::TagHasValue => {
            let [tag] = arguments else { unreachable!() };
            let Expression::Tag { value, .. } = visible.get(*tag) else {
                return None;
            };
            value.is_some().into()
        }
        BuiltinFunction::TagWithoutValue => {
            let [tag] = arguments else { unreachable!() };
            let Expression::Tag { symbol, .. } = visible.get(*tag) else {
                return None;
            };
            Expression::Tag {
                symbol: symbol.clone(),
                value: None,
            }
        }
        BuiltinFunction::TagWithValue => {
            let [tag, value] = arguments else {
                unreachable!()
            };
            let Expression::Tag {
                symbol,
                value: None,
            } = visible.get(*tag)
            else {
                return None;
            };
            Expression::Tag {
                symbol: symbol.clone(),
                value: Some(*value),
            }
        }
        BuiltinFunction::TextCharacters => {
            let [text] = arguments else { unreachable!() };
            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            let mut body = Body::default();
            let characters = text
                .graphemes(true)
                .map(|it| body.push_with_new_id(id_generator, it))
                .collect_vec();
            body.push_with_new_id(id_generator, characters);
            expression.replace_with_multiple(body, pureness);
            return None;
        }
        BuiltinFunction::TextConcatenate => {
            let [a, b] = arguments else { unreachable!() };
            match (visible.get(*a), visible.get(*b)) {
                (Expression::Text(text), other) | (other, Expression::Text(text))
                    if text.is_empty() =>
                {
                    other.clone()
                }
                (Expression::Text(text_a), Expression::Text(text_b)) => {
                    Expression::Text(format!("{text_a}{text_b}"))
                }
                _ => return None,
            }
        }
        BuiltinFunction::TextContains => {
            let [text, pattern] = arguments else {
                unreachable!()
            };
            let Expression::Text(pattern) = visible.get(*pattern) else {
                return None;
            };
            if pattern.is_empty() {
                return Some(true.into());
            }

            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.contains(pattern).into()
        }
        BuiltinFunction::TextEndsWith => {
            let [text, suffix] = arguments else {
                unreachable!()
            };
            let Expression::Text(suffix) = visible.get(*suffix) else {
                return None;
            };
            if suffix.is_empty() {
                return Some(true.into());
            }

            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.ends_with(suffix).into()
        }
        BuiltinFunction::TextFromUtf8 => {
            let [bytes] = arguments else { unreachable!() };
            let Expression::List(bytes) = visible.get(*bytes) else {
                return None;
            };

            // TODO: Remove `u8` checks once we have `needs` ensuring that the bytes are valid.
            let bytes = bytes
                .iter()
                .map(|it| {
                    let Expression::Int(it) = visible.get(*it) else {
                        return Err(());
                    };
                    it.to_u8().ok_or(())
                })
                .try_collect();
            let Ok(bytes) = bytes else {
                return None;
            };

            let mut body = Body::default();
            let result = String::from_utf8(bytes)
                .map(|it| body.push_with_new_id(id_generator, it))
                .map_err(|_| body.push_with_new_id(id_generator, "Invalid UTF-8."));
            body.push_with_new_id(id_generator, result);
            expression.replace_with_multiple(body, pureness);
            return None;
        }
        BuiltinFunction::TextGetRange => {
            let [text, start_inclusive, end_exclusive] = arguments else {
                unreachable!()
            };
            if start_inclusive.semantically_equals(*end_exclusive, visible, pureness) == Some(true)
            {
                return Some("".into());
            }

            let end_exclusive = if let Expression::Int(end_exclusive) = visible.get(*end_exclusive)
            {
                Some(end_exclusive)
            } else {
                None
            };
            if let Some(end_exclusive) = end_exclusive
                && end_exclusive.is_zero()
            {
                return Some("".into());
            }

            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            let Expression::Int(start_inclusive) = visible.get(*start_inclusive) else {
                return None;
            };
            // TODO: Support indices larger than usize.
            let start_inclusive = start_inclusive.to_usize().unwrap();

            if text.graphemes(true).count() == start_inclusive.to_usize().unwrap() {
                return Some("".into());
            }

            let end_exclusive = end_exclusive?.to_usize().unwrap();

            text.graphemes(true)
                .skip(start_inclusive)
                .take(end_exclusive - start_inclusive)
                .collect::<String>()
                .into()
        }
        BuiltinFunction::TextIsEmpty => {
            let [text] = arguments else { unreachable!() };
            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.is_empty().into()
        }
        BuiltinFunction::TextLength => {
            let [text] = arguments else { unreachable!() };
            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.graphemes(true).count().into()
        }
        BuiltinFunction::TextStartsWith => {
            let [text, suffix] = arguments else {
                unreachable!()
            };
            let Expression::Text(suffix) = visible.get(*suffix) else {
                return None;
            };
            if suffix.is_empty() {
                return Some(true.into());
            }

            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.starts_with(suffix).into()
        }
        BuiltinFunction::TextTrimEnd => {
            let [text] = arguments else { unreachable!() };
            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.trim_end().into()
        }
        BuiltinFunction::TextTrimStart => {
            let [text] = arguments else { unreachable!() };
            let Expression::Text(text) = visible.get(*text) else {
                return None;
            };
            text.trim_start().into()
        }
        BuiltinFunction::ToDebugText => {
            let [argument] = arguments else {
                unreachable!()
            };
            let formatted =
                format_value(*argument, Precedence::Low, MaxLength::Unlimited, &|id| {
                    Some(match visible.get(id) {
                        Expression::Int(int) => FormatValue::Int(Cow::Borrowed(int)),
                        Expression::Text(text) => FormatValue::Text(text),
                        Expression::Tag { symbol, value } => FormatValue::Tag {
                            symbol,
                            value: *value,
                        },
                        Expression::Builtin(_) => FormatValue::Function,
                        Expression::List(items) => FormatValue::List(items),
                        Expression::Struct(entries) => FormatValue::Struct(Cow::Borrowed(entries)),
                        Expression::Function { .. } => FormatValue::Function,
                        _ => return None,
                    })
                })?;
            formatted.into()
        }
        BuiltinFunction::TypeOf => Expression::tag(
            match visible.get(arguments[0]) {
                Expression::Int(_) => "Int",
                Expression::Text(_) => "Text",
                Expression::Tag { .. } => "Tag",
                Expression::Builtin(_) => "Function",
                Expression::List(_) => "List",
                Expression::Struct(_) => "Struct",
                Expression::Reference(_) => return None,
                Expression::HirId(_) => unreachable!(),
                Expression::Function { .. } => "Function",
                Expression::Parameter => return None,
                Expression::Call { function, .. } => {
                    let callee = visible.get(*function);
                    let Expression::Builtin(builtin) = callee else {
                        return None;
                    };
                    match builtin {
                        BuiltinFunction::Equals => "Tag",
                        BuiltinFunction::GetArgumentCount => "Int",
                        BuiltinFunction::FunctionRun => return None,
                        BuiltinFunction::IfElse => return None,
                        BuiltinFunction::IntAdd => "Int",
                        BuiltinFunction::IntBitLength => "Int",
                        BuiltinFunction::IntBitwiseAnd => "Int",
                        BuiltinFunction::IntBitwiseOr => "Int",
                        BuiltinFunction::IntBitwiseXor => "Int",
                        BuiltinFunction::IntCompareTo => "Tag",
                        BuiltinFunction::IntDivideTruncating => "Int",
                        BuiltinFunction::IntModulo => "Int",
                        BuiltinFunction::IntMultiply => "Int",
                        BuiltinFunction::IntParse => "Struct",
                        BuiltinFunction::IntRemainder => "Int",
                        BuiltinFunction::IntShiftLeft => "Int",
                        BuiltinFunction::IntShiftRight => "Int",
                        BuiltinFunction::IntSubtract => "Int",
                        BuiltinFunction::ListFilled => "List",
                        BuiltinFunction::ListGet => return None,
                        BuiltinFunction::ListInsert => "List",
                        BuiltinFunction::ListLength => "Int",
                        BuiltinFunction::ListRemoveAt => "List",
                        BuiltinFunction::ListReplace => "List",
                        BuiltinFunction::Print => "Tag",
                        BuiltinFunction::StructGet => return None,
                        BuiltinFunction::StructGetKeys => "List",
                        BuiltinFunction::StructHasKey => "Tag",
                        BuiltinFunction::TagGetValue => return None,
                        BuiltinFunction::TagHasValue => "Tag",
                        BuiltinFunction::TagWithoutValue => "Tag",
                        BuiltinFunction::TagWithValue => "Tag",
                        BuiltinFunction::TextCharacters => "List",
                        BuiltinFunction::TextConcatenate => "Text",
                        BuiltinFunction::TextContains => "Tag",
                        BuiltinFunction::TextEndsWith => "Tag",
                        BuiltinFunction::TextFromUtf8 => "Tag",
                        BuiltinFunction::TextGetRange => "Text",
                        BuiltinFunction::TextIsEmpty => "Tag",
                        BuiltinFunction::TextLength => "Int",
                        BuiltinFunction::TextStartsWith => "Tag",
                        BuiltinFunction::TextTrimEnd => "Text",
                        BuiltinFunction::TextTrimStart => "Text",
                        BuiltinFunction::ToDebugText => "Text",
                        BuiltinFunction::TypeOf => "Tag",
                    }
                }
                Expression::UseModule { .. } => return None,
                Expression::Panic { .. } => return None,
                Expression::TraceCallStarts { .. }
                | Expression::TraceCallEnds { .. }
                | Expression::TraceTailCall { .. }
                | Expression::TraceExpressionEvaluated { .. }
                | Expression::TraceFoundFuzzableFunction { .. } => unreachable!(),
            }
            .to_string(),
        ),
    };
    Some(result)
}

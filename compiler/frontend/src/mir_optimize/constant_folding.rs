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

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::{
    builtin_functions::BuiltinFunction,
    id::IdGenerator,
    mir::{Body, Expression, Id, VisibleExpressions},
    rich_ir::ToRichIr,
};

pub fn fold_constants(
    expression: &mut Expression,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
) {
    let Expression::Call {
        function,
        arguments,
        responsible,
    } = expression else { return; };

    if let Expression::Tag { symbol, value: None } = visible.get(*function) && arguments.len() == 1 {
        *expression = Expression::Tag { symbol: *symbol, value: Some(arguments[0]) };
        return;
    }

    let Expression::Builtin(builtin) = visible.get(*function) else { return; };
    let Some(result) = run_builtin(*builtin, arguments, *responsible, visible, id_generator) else {
        return;
    };
    let evaluated_call = match result {
        BuiltinResult::Returns(expression) => expression,
        BuiltinResult::Panics(reason) => {
            let mut body = Body::default();
            let reason = body.push_with_new_id(id_generator, Expression::Text(reason));
            body.push_with_new_id(
                id_generator,
                Expression::Panic {
                    reason,
                    responsible: *responsible,
                },
            );
            Expression::Multiple(body)
        }
    };
    *expression = evaluated_call;
}

enum BuiltinResult {
    Returns(Expression),
    Panics(String),
}

/// This function tries to run a builtin, requiring a minimal amount of static
/// knowledge. For example, it can find out that the result of
/// `builtinEquals $3 $3` is `True`, even if the value of `$3` is not known at
/// compile-time.
///
/// Returns `None` if the call couldn't be evaluated statically. Returns
/// `Some(Ok(expression))` if the call successfully completed with a return
/// value. Returns `Some(Err(reason))` if the call panics.
fn run_builtin(
    builtin: BuiltinFunction,
    arguments: &[Id],
    responsible: Id,
    visible: &VisibleExpressions,
    id_generator: &mut IdGenerator<Id>,
) -> Option<BuiltinResult> {
    use BuiltinResult::*;

    if arguments.len() != builtin.num_parameters() {
        return Panics("wrong number of arguments".to_string()).into();
    }

    let result = match builtin {
        BuiltinFunction::ChannelCreate
        | BuiltinFunction::ChannelSend
        | BuiltinFunction::ChannelReceive => return None,
        BuiltinFunction::Equals => {
            let [a, b] = arguments else { unreachable!() };
            Returns(a.semantically_equals(*b, visible)?.into())
        }
        BuiltinFunction::FunctionRun => {
            let [function] = arguments else { unreachable!() };
            Returns(Expression::Call {
                function: *function,
                arguments: vec![],
                responsible,
            })
        }
        BuiltinFunction::GetArgumentCount => {
            let [function] = arguments else { unreachable!() };
            let Expression::Function { parameters, .. } = visible.get(*function) else { return None; };
            Returns(Expression::Int(parameters.len().into()))
        }
        BuiltinFunction::IfElse => {
            let [condition, then_body, else_body] = arguments else { unreachable!() };
            let Expression::Tag { symbol, value: None } = visible.get(*condition) else { return None; };
            let Expression::Symbol(symbol) = visible.get(*symbol) else { return None; };
            let condition = match symbol.as_str() {
                "True" => true,
                "False" => false,
                _ => return None,
            };
            Returns(Expression::Call {
                function: if condition { *then_body } else { *else_body },
                arguments: vec![],
                responsible,
            })
        }
        BuiltinFunction::IntAdd => {
            let [a, b] = arguments else { unreachable!() };
            let a: BigInt = visible.get(*a).try_into().ok()?;
            let b: BigInt = visible.get(*b).try_into().ok()?;
            Returns(Expression::Int(a + b))
        }
        BuiltinFunction::IntBitLength => {
            let [a] = arguments else { unreachable!() };
            let a: BigInt = visible.get(*a).try_into().ok()?;
            Returns(Expression::Int(a.bits().into()))
        }
        // TODO: Implement
        BuiltinFunction::IntBitwiseAnd => return None,
        BuiltinFunction::IntBitwiseOr => return None,
        BuiltinFunction::IntBitwiseXor => return None,
        BuiltinFunction::IntCompareTo => return None,
        BuiltinFunction::IntDivideTruncating => return None,
        BuiltinFunction::IntModulo => return None,
        BuiltinFunction::IntMultiply => return None,
        BuiltinFunction::IntParse => return None,
        BuiltinFunction::IntRemainder => return None,
        BuiltinFunction::IntShiftLeft => return None,
        BuiltinFunction::IntShiftRight => return None,
        BuiltinFunction::IntSubtract => return None,
        BuiltinFunction::ListFilled => return None,
        BuiltinFunction::ListGet => {
            let [list, index] = arguments else { unreachable!() };

            // TODO: Also catch this being called on a non-list and
            // statically panic in that case.
            let Expression::List(list) = visible.get(*list) else {
                return None;
            };

            // TODO: Also catch this being called on a non-int and
            // statically panic in that case.
            let Expression::Int(index) = visible.get(*index) else {
                return None;
            };

            let Some(value) = index.to_usize().and_then(|index| list.get(index)) else {
                return Some(Panics(format!("List access will panic because index {} is out of bounds.", index)));
            };
            Returns(Expression::Reference(*value))
        }
        BuiltinFunction::ListInsert => return None,
        BuiltinFunction::ListLength => {
            let [list_id] = arguments else { unreachable!() };

            // TODO: Also catch this being called on a non-list and
            // statically panic in that case.
            let Expression::List(list) = visible.get(*list_id) else {
                return None;
            };

            Returns(Expression::Int(list.len().into()))
        }
        BuiltinFunction::ListRemoveAt => return None,
        BuiltinFunction::ListReplace => return None,
        BuiltinFunction::Parallel => return None,
        BuiltinFunction::Print => return None,
        BuiltinFunction::StructGet => {
            let [struct_, key] = arguments else { unreachable!() };

            // TODO: Also catch this being called on a non-struct and
            // statically panic in that case.
            let Expression::Struct(fields) = visible.get(*struct_) else {
                return None;
            };

            // TODO: Relax this requirement. Even if not all keys are
            // constant, we may still conclude the result of the builtin:
            // If one key `semantically_equals` the requested one and all
            // others definitely not, then we can still resolve that.
            if !visible.get(*key).is_constant(visible) {
                return None;
            }
            if fields
                .iter()
                .any(|(key, _)| !visible.get(*key).is_constant(visible))
            {
                return None;
            }

            let value = fields
                .iter()
                .rev()
                .find(|(k, _)| k.semantically_equals(*key, visible).unwrap_or(false))
                .map(|(_, value)| *value);
            if let Some(value) = value {
                Returns(Expression::Reference(value))
            } else {
                return Some(Panics(format!(
                    "Struct access will panic because key {} isn't in there.",
                    visible.get(*key).to_rich_ir(),
                )));
            }
        }
        BuiltinFunction::StructGetKeys => return None,
        BuiltinFunction::StructHasKey => {
            let [struct_, key] = arguments else { unreachable!() };

            // TODO: Also catch this being called on a non-struct and
            // statically panic in that case.
            let Expression::Struct(fields) = visible.get(*struct_) else {
                return None;
            };

            let mut is_contained = Some(false);
            for (k, _) in fields.iter() {
                match k.semantically_equals(*key, visible) {
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

            Returns(is_contained?.into())
        }
        BuiltinFunction::TagGetValue => return None,
        BuiltinFunction::TagHasValue => return None,
        BuiltinFunction::TagWithoutValue => {
            let [tag] = arguments else { unreachable!() };
            let Expression::Tag { symbol, .. } = visible.get(*tag) else { return None; };

            Returns(Expression::Tag {
                symbol: *symbol,
                value: None,
            })
        }
        BuiltinFunction::TextCharacters => return None,
        BuiltinFunction::TextConcatenate => {
            let [a, b] = arguments else { unreachable!() };

            Returns(match (visible.get(*a), visible.get(*b)) {
                (Expression::Text(text), other) | (other, Expression::Text(text))
                    if text.is_empty() =>
                {
                    other.clone()
                }
                (Expression::Text(text_a), Expression::Text(text_b)) => {
                    Expression::Text(format!("{}{}", text_a, text_b))
                }
                _ => return None,
            })
        }
        BuiltinFunction::TextContains => return None,
        BuiltinFunction::TextEndsWith => return None,
        BuiltinFunction::TextFromUtf8 => return None,
        BuiltinFunction::TextGetRange => return None,
        BuiltinFunction::TextIsEmpty => return None,
        BuiltinFunction::TextLength => return None,
        BuiltinFunction::TextStartsWith => return None,
        BuiltinFunction::TextTrimEnd => return None,
        BuiltinFunction::TextTrimStart => return None,
        BuiltinFunction::ToDebugText => return None,
        BuiltinFunction::Try => return None,
        BuiltinFunction::TypeOf => Returns(Expression::tag(
            id_generator,
            match visible.get(arguments[0]) {
                Expression::Int(_) => "Int",
                Expression::Text(_) => "Text",
                Expression::Symbol(_) => unreachable!(),
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
                        BuiltinFunction::ChannelCreate => "Struct",
                        BuiltinFunction::ChannelSend => "Tag",
                        BuiltinFunction::ChannelReceive => return None,
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
                        BuiltinFunction::Parallel => return None,
                        BuiltinFunction::Print => "Tag",
                        BuiltinFunction::StructGet => return None,
                        BuiltinFunction::StructGetKeys => "List",
                        BuiltinFunction::StructHasKey => "Tag",
                        BuiltinFunction::TagGetValue => return None,
                        BuiltinFunction::TagHasValue => "Tag",
                        BuiltinFunction::TagWithoutValue => "Tag",
                        BuiltinFunction::TextCharacters => "List",
                        BuiltinFunction::TextConcatenate => "Text",
                        BuiltinFunction::TextContains => "Tag",
                        BuiltinFunction::TextEndsWith => "Tag",
                        BuiltinFunction::TextFromUtf8 => "Struct",
                        BuiltinFunction::TextGetRange => "Text",
                        BuiltinFunction::TextIsEmpty => "Tag",
                        BuiltinFunction::TextLength => "Int",
                        BuiltinFunction::TextStartsWith => "Tag",
                        BuiltinFunction::TextTrimEnd => "Text",
                        BuiltinFunction::TextTrimStart => "Text",
                        BuiltinFunction::ToDebugText => "Text",
                        BuiltinFunction::Try => "Struct",
                        BuiltinFunction::TypeOf => "Tag",
                    }
                }
                Expression::UseModule { .. } => return None,
                Expression::Panic { .. } => return None,
                Expression::Multiple(_) => return None,
                Expression::TraceCallStarts { .. }
                | Expression::TraceCallEnds { .. }
                | Expression::TraceExpressionEvaluated { .. }
                | Expression::TraceFoundFuzzableFunction { .. } => unreachable!(),
            }
            .to_string(),
            None,
        )),
    };
    Some(result)
}

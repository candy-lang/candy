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
    mir::{Body, Expression, Id, Mir, VisibleExpressions},
    rich_ir::ToRichIr,
};

impl Mir {
    pub fn fold_constants(&mut self) {
        self.body
            .visit_with_visible(&mut |_, expression, visible, _| {
                let Expression::Call {
                    function,
                    arguments,
                    responsible,
                } = expression else { return; };
                let Expression::Builtin(builtin) = visible.get(*function) else { return; };
                let Some(result) = run_builtin(*builtin, arguments, *responsible, visible) else {
                    return;
                };
                let evaluated_call = match result {
                    Ok(return_value) => return_value,
                    Err(panic_reason) => {
                        let mut body = Body::default();
                        let reason = body.push_with_new_id(
                            &mut self.id_generator,
                            Expression::Text(panic_reason),
                        );
                        body.push_with_new_id(
                            &mut self.id_generator,
                            Expression::Panic {
                                reason,
                                responsible: *responsible,
                            },
                        );
                        Expression::Multiple(body)
                    }
                };
                *expression = evaluated_call;
            });
    }
}

/// This function tries to run a builtin, requiring a minimal amount of
/// static knowledge. For example, it can find out that the result of
/// `builtinEquals $3 $3` is `True`, even if the value of `$3` is not known
/// at compile-time.
///
/// Returns `None` if the call couldn't be evaluated statically. Returns
/// `Some(Ok(expression))` if the call successfully completed with a return
/// value. Returns `Some(Err(reason))` if the call panics.
fn run_builtin(
    builtin: BuiltinFunction,
    arguments: &[Id],
    responsible: Id,
    visible: &VisibleExpressions,
) -> Option<Result<Expression, String>> {
    if arguments.len() != builtin.num_parameters() {
        return Some(Err("wrong number of arguments".to_string()));
    }

    let return_value = match builtin {
        BuiltinFunction::ChannelCreate
        | BuiltinFunction::ChannelSend
        | BuiltinFunction::ChannelReceive => return None,
        BuiltinFunction::Equals => {
            let [a, b] = arguments else { unreachable!() };
            let are_equal = a.semantically_equals(*b, visible)?;
            Expression::Symbol(if are_equal { "True" } else { "False" }.to_string())
        }
        BuiltinFunction::FunctionRun => {
            let [lambda] = arguments else { unreachable!() };
            Expression::Call {
                function: arguments[0],
                arguments: vec![],
                responsible,
            }
        }
        BuiltinFunction::GetArgumentCount => {
            let [lambda] = arguments else { unreachable!() };
            let Expression::Lambda { parameters, .. } = visible.get(arguments[0]) else { return None; };
            Expression::Int(parameters.len().into())
        }
        BuiltinFunction::IfElse => {
            let [condition, then_body, else_body] = arguments else { unreachable!() };
            let condition = visible.get(*condition).try_into().ok()?;
            Expression::Call {
                function: if condition { *then_body } else { *else_body },
                arguments: vec![],
                responsible,
            }
        }
        BuiltinFunction::IntAdd => {
            let [a, b] = arguments else { unreachable!() };
            let a: BigInt = visible.get(*a).try_into().ok()?;
            let b: BigInt = visible.get(*b).try_into().ok()?;
            Expression::Int(a + b)
        }
        BuiltinFunction::IntBitLength => {
            let [a] = arguments else { unreachable!() };
            let a: BigInt = visible.get(*a).try_into().ok()?;
            Expression::Int(a.bits().into())
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
                return Some(Err(format!("List access will panic because index {} is out of bounds.", index)));
            };
            Expression::Reference(*value)
        }
        BuiltinFunction::ListInsert => return None,
        BuiltinFunction::ListLength => {
            let [list_id] = arguments else { unreachable!() };

            // TODO: Also catch this being called on a non-list and
            // statically panic in that case.
            let Expression::List(list) = visible.get(*list_id) else {
                return None;
            };

            Expression::Int(list.len().into())
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
                Expression::Reference(value)
            } else {
                return Some(Err(format!(
                    "Struct access will panic because key {:?} isn't in there.",
                    visible.get(*key).to_rich_ir(),
                )));
            }
        }
        BuiltinFunction::StructGetKeys => return None,
        BuiltinFunction::StructHasKey => return None,
        BuiltinFunction::TextCharacters => return None,
        BuiltinFunction::TextConcatenate => {
            let [a, b] = arguments else { unreachable!() };

            match (visible.get(*a), visible.get(*b)) {
                (Expression::Text(text), other) | (other, Expression::Text(text))
                    if text.is_empty() =>
                {
                    other.clone()
                }
                (Expression::Text(text_a), Expression::Text(text_b)) => {
                    Expression::Text(format!("{}{}", text_a, text_b))
                }
                _ => return None,
            }
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
        BuiltinFunction::TypeOf => match visible.get(arguments[0]) {
            Expression::Int(_) => Expression::Symbol("Int".to_string()),
            Expression::Text(_) => Expression::Symbol("Text".to_string()),
            Expression::Symbol(_) => Expression::Symbol("Symbol".to_string()),
            Expression::Builtin(_) => Expression::Symbol("Function".to_string()),
            Expression::List(_) => Expression::Symbol("List".to_string()),
            Expression::Struct(_) => Expression::Symbol("Struct".to_string()),
            Expression::Reference(_) => return None,
            Expression::HirId(_) => unreachable!(),
            Expression::Lambda { .. } => Expression::Symbol("Function".to_string()),
            Expression::Parameter => return None,
            Expression::Call { function, .. } => {
                let callee = visible.get(*function);
                let Expression::Builtin(builtin) = callee else {
                        return None;
                    };
                let return_type = match builtin {
                    BuiltinFunction::ChannelCreate => "Struct",
                    BuiltinFunction::ChannelSend => "Symbol",
                    BuiltinFunction::ChannelReceive => return None,
                    BuiltinFunction::Equals => "Symbol",
                    BuiltinFunction::GetArgumentCount => "Int",
                    BuiltinFunction::FunctionRun => return None,
                    BuiltinFunction::IfElse => return None,
                    BuiltinFunction::IntAdd => "Int",
                    BuiltinFunction::IntBitLength => "Int",
                    BuiltinFunction::IntBitwiseAnd => "Int",
                    BuiltinFunction::IntBitwiseOr => "Int",
                    BuiltinFunction::IntBitwiseXor => "Int",
                    BuiltinFunction::IntCompareTo => "Symbol",
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
                    BuiltinFunction::Print => "Symbol",
                    BuiltinFunction::StructGet => return None,
                    BuiltinFunction::StructGetKeys => "List",
                    BuiltinFunction::StructHasKey => "Symbol",
                    BuiltinFunction::TextCharacters => "List",
                    BuiltinFunction::TextConcatenate => "Text",
                    BuiltinFunction::TextContains => "Symbol",
                    BuiltinFunction::TextEndsWith => "Symbol",
                    // TODO before merge
                    BuiltinFunction::TextFromUtf8 => return None,
                    BuiltinFunction::TextGetRange => "Text",
                    BuiltinFunction::TextIsEmpty => "Symbol",
                    BuiltinFunction::TextLength => "Int",
                    BuiltinFunction::TextStartsWith => "Symbol",
                    BuiltinFunction::TextTrimEnd => "Text",
                    BuiltinFunction::TextTrimStart => "Text",
                    BuiltinFunction::ToDebugText => "Text",
                    // TODO before merge
                    BuiltinFunction::Try => return None,
                    BuiltinFunction::TypeOf => "Symbol",
                };
                Expression::Symbol(return_type.to_string())
            }
            Expression::UseModule { .. } => return None,
            Expression::Panic { .. } => return None,
            Expression::Multiple(_) => return None,
            Expression::ModuleStarts { .. }
            | Expression::ModuleEnds
            | Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableClosure { .. } => unreachable!(),
        },
    };
    Some(Ok(return_value))
}

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

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::mir::{Body, Expression, Id, Mir, VisibleExpressions},
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
                let Some(result) = Self::run_builtin(*builtin, arguments, *responsible, visible) else {
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
                let are_equal = arguments[0].semantically_equals(arguments[1], visible)?;
                Expression::Symbol(if are_equal { "True" } else { "False" }.to_string())
            }
            BuiltinFunction::FunctionRun => Expression::Call {
                function: arguments[0],
                arguments: vec![],
                responsible,
            },
            BuiltinFunction::GetArgumentCount => {
                let Expression::Lambda { parameters, .. } = visible.get(arguments[0]) else { return None; };
                Expression::Int(parameters.len().into())
            }
            BuiltinFunction::IfElse => {
                let condition = visible.get(arguments[0]).try_into().ok()?;
                let then_body = arguments[1];
                let else_body = arguments[2];
                Expression::Call {
                    function: if condition { then_body } else { else_body },
                    arguments: vec![],
                    responsible,
                }
            }
            BuiltinFunction::IntAdd => {
                let a: BigInt = visible.get(arguments[0]).try_into().ok()?;
                let b: BigInt = visible.get(arguments[1]).try_into().ok()?;
                Expression::Int(a + b)
            }
            BuiltinFunction::IntBitLength => {
                let a: BigInt = visible.get(arguments[0]).try_into().ok()?;
                Expression::Int(a.bits().into())
            }
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
            BuiltinFunction::ListGet => return None,
            BuiltinFunction::ListInsert => return None,
            BuiltinFunction::ListLength => return None,
            BuiltinFunction::ListRemoveAt => return None,
            BuiltinFunction::ListReplace => return None,
            BuiltinFunction::Parallel => return None,
            BuiltinFunction::Print => return None,
            BuiltinFunction::StructGet => {
                let struct_id = arguments[0];
                let key_id = arguments[1];

                // TODO: Also catch this being called on a non-struct and
                // statically panic in that case.
                let Expression::Struct(fields) = visible.get(struct_id) else {
                    return None;
                };

                // TODO: Relax this requirement. Even if not all keys are
                // constant, we may still conclude the result of the builtin:
                // If one key `semantically_equals` the requested one and all
                // others are definitely not, then we can still resolve that.
                if !visible.get(key_id).is_constant(visible) {
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
                    .find(|(k, _)| k.semantically_equals(key_id, visible).unwrap_or(false))
                    .map(|(_, value)| *value);
                if let Some(value) = value {
                    Expression::Reference(value)
                } else {
                    return Some(Err(format!(
                        "Struct access will panic because key {:?} isn't in there.",
                        visible.get(key_id),
                    )));
                }
            }
            BuiltinFunction::StructGetKeys => return None,
            BuiltinFunction::StructHasKey => return None,
            BuiltinFunction::TextCharacters => return None,
            BuiltinFunction::TextConcatenate => {
                let [a, b] = arguments else {
                    return Some(Err("wrong number of arguments".to_string()));
                };

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
                        BuiltinFunction::Equals => "Symbol",
                        BuiltinFunction::GetArgumentCount => "Int",
                        BuiltinFunction::IntAdd => "Int",
                        BuiltinFunction::IntBitLength => "Int",
                        BuiltinFunction::IntBitwiseAnd => "Int",
                        BuiltinFunction::IntBitwiseOr => "Int",
                        BuiltinFunction::IntBitwiseXor => "Int",
                        BuiltinFunction::IntCompareTo => "Symbol",
                        BuiltinFunction::IntDivideTruncating => "Int",
                        BuiltinFunction::IntModulo => "Int",
                        BuiltinFunction::IntMultiply => "Int",
                        BuiltinFunction::IntRemainder => "Int",
                        BuiltinFunction::IntShiftLeft => "Int",
                        BuiltinFunction::IntShiftRight => "Int",
                        BuiltinFunction::IntSubtract => "Int",
                        BuiltinFunction::ListFilled => "List",
                        BuiltinFunction::ListInsert => "List",
                        BuiltinFunction::ListLength => "Int",
                        BuiltinFunction::ListRemoveAt => "List",
                        BuiltinFunction::ListReplace => "List",
                        BuiltinFunction::StructHasKey => "Symbol",
                        BuiltinFunction::TextCharacters => "List",
                        BuiltinFunction::TextConcatenate => "Text",
                        BuiltinFunction::TextContains => "Symbol",
                        BuiltinFunction::TextEndsWith => "Symbol",
                        BuiltinFunction::TextGetRange => "Text",
                        BuiltinFunction::TextIsEmpty => "Symbol",
                        BuiltinFunction::TextLength => "Int",
                        BuiltinFunction::TextStartsWith => "Symbol",
                        BuiltinFunction::TextTrimEnd => "Text",
                        BuiltinFunction::TextTrimStart => "Text",
                        BuiltinFunction::TypeOf => "Symbol",
                        _ => return None,
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
}

use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{Body, Expression, Id},
};
use tracing::warn;

impl Body {
    pub fn fold_constants(&mut self) {
        for id in self.ids.clone() {
            match self.expressions.get_mut(&id).unwrap() {
                Expression::Lambda(lambda) => lambda.body.fold_constants(),
                Expression::Call {
                    function,
                    arguments,
                } => {
                    let function = function.clone();
                    let arguments = arguments.clone();

                    let Some(Expression::Builtin(function)) = self.find(&function) else { continue; };
                    warn!("Constant folding candidate: builtin{function:?}");
                    match function {
                        BuiltinFunction::Equals => {
                            if arguments.len() != 2 {
                                // TODO: panic
                                continue;
                            }

                            let a = arguments[0].clone();
                            let b = arguments[1].clone();

                            let mut are_equal = a == b;
                            if !are_equal {
                                let Some(comparison) = self.equals(&a, &b) else { continue };
                                are_equal = comparison;
                            };

                            self.expressions.insert(
                                id,
                                Expression::Symbol(
                                    if are_equal { "True" } else { "False" }.to_string(),
                                ),
                            );
                        }
                        // BuiltinFunction::FunctionRun => continue,
                        // BuiltinFunction::GetArgumentCount => todo!(),
                        // BuiltinFunction::IfElse => todo!(),
                        // BuiltinFunction::IntAdd => todo!(),
                        // BuiltinFunction::IntBitLength => todo!(),
                        // BuiltinFunction::IntBitwiseAnd => todo!(),
                        // BuiltinFunction::IntBitwiseOr => todo!(),
                        // BuiltinFunction::IntBitwiseXor => todo!(),
                        // BuiltinFunction::IntCompareTo => todo!(),
                        // BuiltinFunction::IntDivideTruncating => todo!(),
                        // BuiltinFunction::IntModulo => todo!(),
                        // BuiltinFunction::IntMultiply => todo!(),
                        // BuiltinFunction::IntParse => todo!(),
                        // BuiltinFunction::IntRemainder => todo!(),
                        // BuiltinFunction::IntShiftLeft => todo!(),
                        // BuiltinFunction::IntShiftRight => todo!(),
                        // BuiltinFunction::IntSubtract => todo!(),
                        // BuiltinFunction::Parallel => todo!(),
                        // BuiltinFunction::Print => todo!(),
                        BuiltinFunction::StructGet => {
                            if arguments.len() != 2 {
                                // TODO: panic
                                continue;
                            }

                            let struct_id = arguments[0].clone();
                            let key_id = arguments[1].clone();

                            let Expression::Struct(fields) = self.find(&struct_id).unwrap() else { warn!("builtinStructGet called on non-struct"); continue; };
                            let key = self.find(&key_id).unwrap();

                            if fields.keys().all(|key| self.is_constant(key))
                                && self.is_constant(&key_id)
                            {
                                let value = fields
                                    .iter()
                                    .find(|(k, _)| self.equals(&key_id, k).unwrap_or(false))
                                    .map(|(_, value)| value.clone());
                                if let Some(value) = value {
                                    self.expressions
                                        .insert(id, Expression::Reference(value.clone()));
                                } else {
                                    // panic
                                    // self.expressions.insert(id, Expression::Panic {})
                                    // todo!("Struct access will panic.")
                                    continue;
                                }
                            } else {
                                warn!("Not all keys are constant.");
                            }
                        }
                        // BuiltinFunction::StructGetKeys => todo!(),
                        // BuiltinFunction::StructHasKey => todo!(),
                        // BuiltinFunction::TextCharacters => todo!(),
                        // BuiltinFunction::TextConcatenate => todo!(),
                        // BuiltinFunction::TextContains => todo!(),
                        // BuiltinFunction::TextEndsWith => todo!(),
                        // BuiltinFunction::TextGetRange => todo!(),
                        // BuiltinFunction::TextIsEmpty => todo!(),
                        // BuiltinFunction::TextLength => todo!(),
                        // BuiltinFunction::TextStartsWith => todo!(),
                        // BuiltinFunction::TextTrimEnd => todo!(),
                        // BuiltinFunction::TextTrimStart => todo!(),
                        // BuiltinFunction::Try => todo!(),
                        // BuiltinFunction::TypeOf => todo!(),
                        _ => continue,
                    }
                }
                Expression::UseModule {
                    current_module,
                    relative_path,
                } => {
                    // TODO: Check if the relative path is const and insert the
                    // code.
                }
                Expression::Needs { condition, reason } => {
                    // TODO: Check if the condition is const. If it's true,
                    // remove the need. Otherwise, replace it with a panic.
                }
                Expression::Error { child, errors } => {
                    // TODO: Remove and replace with a panic.
                }
                _ => {}
            }
        }
    }
}

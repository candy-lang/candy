use crate::{
    builtin_functions::BuiltinFunction,
    compiler::mir::{Expression, Id, Mir},
};
use itertools::Itertools;
use std::collections::HashMap;
use tracing::{debug, warn};

impl Mir {
    pub fn fold_constants(&mut self) {
        Self::fold_inner_constants(&mut self.expressions, &mut self.body);
    }
    fn fold_inner_constants(expressions: &mut HashMap<Id, Expression>, body: &mut Vec<Id>) {
        for id in body {
            let mut temporary = id.temporarily_get_mut(expressions);

            match &mut temporary.expression {
                Expression::Lambda { body, .. } => {
                    Self::fold_inner_constants(&mut temporary.remaining, body);
                }
                Expression::Call {
                    function,
                    arguments,
                    responsible,
                } => {
                    if let Some(Expression::Builtin(builtin)) = temporary.remaining.get(&function) &&
                        let Some(expression) = Self::run_builtin(*builtin, arguments, &temporary.remaining)
                    {
                        temporary.remaining.insert(*id, expression);
                        debug!("Builtin {id} inlined to {}.", id.format(temporary.remaining));
                    }
                }
                Expression::Needs { condition, reason, responsible } => {
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

    fn run_builtin(
        builtin: BuiltinFunction,
        arguments: &[Id],
        expressions: &HashMap<Id, Expression>,
    ) -> Option<Expression> {
        // warn!("Constant folding candidate: builtin{builtin:?}");
        // warn!(
        //     "Arguments: {}",
        //     arguments.iter().map(|arg| format!("{arg}")).join(", ")
        // );
        // warn!(
        //     "Expressions:\n{}",
        //     expressions
        //         .iter()
        //         .map(|(id, expr)| format!("{id}: {expr}"))
        //         .join("\n")
        // );

        Some(match builtin {
            BuiltinFunction::Equals => {
                if arguments.len() != 2 {
                    // TODO: panic
                    return None;
                }

                let a = arguments[0];
                let b = arguments[1];

                let mut are_equal = a == b;
                if !are_equal {
                    let Some(comparison) = a.semantically_equals(b, expressions) else { return None; };
                    are_equal = comparison;
                };

                Expression::Symbol(if are_equal { "True" } else { "False" }.to_string())
            }
            // BuiltinFunction::FunctionRun => return,
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
                    return None;
                }

                let struct_id = arguments[0].clone();
                let key_id = arguments[1].clone();

                let Some(Expression::Struct(fields)) = expressions.get(&struct_id) else {
                    // warn!("builtinStructGet called with non-constant struct");
                    return None;
                };

                if fields.keys().all(|key| key.is_constant(expressions))
                    && key_id.is_constant(expressions)
                {
                    let value = fields
                        .iter()
                        .find(|(k, _)| k.semantically_equals(key_id, expressions).unwrap_or(false))
                        .map(|(_, value)| value.clone());
                    if let Some(value) = value {
                        Expression::Reference(value.clone())
                    } else {
                        // panic
                        warn!("Struct access will panic.");
                        return None;
                    }
                } else {
                    warn!("Not all keys are constant.");
                    return None;
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
            _ => return None,
        })
    }
}

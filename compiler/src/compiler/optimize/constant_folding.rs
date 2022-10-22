use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{Body, Expression, Id},
};
use im::HashMap;
use itertools::Itertools;
use tracing::warn;

impl Body {
    pub fn fold_constants(&mut self) {
        self.fold_inner_constants(HashMap::new());
    }

    fn fold_inner_constants(&mut self, mut outer_expressions: HashMap<Id, Expression>) {
        for id in self.ids.clone() {
            let mut expression = self.expressions.get(&id).unwrap().clone();
            match &mut expression {
                Expression::Lambda(lambda) => {
                    lambda.body.fold_inner_constants(outer_expressions.clone());
                }
                Expression::Call {
                    function,
                    arguments,
                } => {
                    let function = function.clone();
                    let arguments = arguments.clone();

                    if let Some(Expression::Builtin(builtin)) = self.find(&function) &&
                        let Some(expression) = Self::run_builtin(*builtin, arguments, &outer_expressions)
                    {
                        *self.expressions.get_mut(&id).unwrap() = expression.clone();
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
            outer_expressions.insert(id.clone(), expression);
        }
    }

    fn run_builtin(
        builtin: BuiltinFunction,
        arguments: Vec<Id>,
        expressions: &HashMap<Id, Expression>,
    ) -> Option<Expression> {
        warn!("Constant folding candidate: builtin{builtin:?}");
        warn!(
            "Arguments: {}",
            arguments.iter().map(|arg| format!("{arg}")).join(", ")
        );
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

                let a = arguments[0].clone();
                let b = arguments[1].clone();

                let mut are_equal = a == b;
                if !are_equal {
                    let Some(comparison) = equals(expressions, &a, &b) else { return None; };
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
                    warn!("builtinStructGet called on non-struct");
                    return None;
                };
                let key = expressions.get(&key_id).unwrap();

                if fields.keys().all(|key| is_constant(expressions, key))
                    && is_constant(expressions, &key_id)
                {
                    let value = fields
                        .iter()
                        .find(|(k, _)| equals(expressions, &key_id, k).unwrap_or(false))
                        .map(|(_, value)| value.clone());
                    if let Some(value) = value {
                        Expression::Reference(value.clone())
                    } else {
                        // panic
                        // self.expressions.insert(id, Expression::Panic {})
                        // todo!("Struct access will panic.")
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

fn equals(expressions: &HashMap<Id, Expression>, a: &Id, b: &Id) -> Option<bool> {
    if a == b {
        return Some(true);
    }
    let a_expr = expressions.get(a).unwrap();
    let b_expr = expressions.get(a).unwrap();
    if let Expression::Reference(reference) = a_expr {
        return equals(expressions, reference, b);
    }
    if let Expression::Reference(reference) = b_expr {
        return equals(expressions, a, reference);
    }

    match (expressions.get(a).unwrap(), expressions.get(b).unwrap()) {
        (Expression::Int(a), Expression::Int(b)) => Some(a == b),
        (Expression::Text(a), Expression::Text(b)) => Some(a == b),
        (Expression::Symbol(a), Expression::Symbol(b)) => Some(a == b),
        (Expression::Struct(a), Expression::Struct(b)) => {
            // TODO
            todo!()
        }
        // Also consider lambdas equal where only some IDs are named
        // differently.
        (Expression::Lambda(a), Expression::Lambda(b)) => Some(a == b),
        (Expression::Builtin(a), Expression::Builtin(b)) => Some(a == b),
        (Expression::Call { .. }, _)
        | (Expression::UseModule { .. }, _)
        | (Expression::Needs { .. }, _)
        | (Expression::Error { .. }, _)
        | (_, Expression::Call { .. })
        | (_, Expression::UseModule { .. })
        | (_, Expression::Needs { .. })
        | (_, Expression::Error { .. }) => None,
        (_, _) => Some(false),
    }
}

pub fn is_constant(expressions: &HashMap<Id, Expression>, id: &Id) -> bool {
    match &expressions.get(id).unwrap() {
        Expression::Int(_)
        | Expression::Text(_)
        | Expression::Symbol(_)
        | Expression::Builtin(_) => true,
        Expression::Reference(id) => is_constant(expressions, &id),
        Expression::Struct(fields) => fields
            .iter()
            .all(|(key, value)| is_constant(expressions, key) && is_constant(expressions, value)),
        Expression::Lambda(lambda) => lambda
            .captured_ids(id)
            .iter()
            .all(|id| is_constant(expressions, id)),
        Expression::Call { .. }
        | Expression::UseModule { .. }
        | Expression::Needs { .. }
        | Expression::Error { .. } => false,
    }
}

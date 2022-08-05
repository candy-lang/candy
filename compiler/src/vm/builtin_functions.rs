use std::{cmp::Ordering, str::FromStr};

use super::{
    heap::ObjectPointer,
    use_provider::UseProvider,
    value::{Closure, Value},
    Vm,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use itertools::Itertools;
use num_bigint::{BigInt, ToBigInt};
use num_integer::Integer;
use num_traits::ToPrimitive;
use unicode_segmentation::UnicodeSegmentation;

macro_rules! destructure {
    ($args:expr, $enum:pat, $body:block) => {{
        if let $enum = &$args[..] {
            $body
        } else {
            Err(format!("a builtin function received invalid arguments"))
        }
    }};
}

impl Vm {
    pub(super) fn run_builtin_function<U: UseProvider>(
        &mut self,
        use_provider: &U,
        builtin_function: &BuiltinFunction,
        args: &[ObjectPointer],
    ) {
        log::trace!("run_builtin_function: builtin{builtin_function:?}");

        let args = args.iter().map(|it| self.heap.export(*it)).collect_vec();

        let return_value_or_panic_reason = match &builtin_function {
            BuiltinFunction::Equals => self.equals(args),
            BuiltinFunction::FunctionRun => match self.function_run(use_provider, args) {
                // If successful, `functionRun` doesn't return a value, but
                // diverges the control flow.
                Ok(()) => return,
                Err(message) => Err(message),
            },
            BuiltinFunction::GetArgumentCount => self.get_argument_count(args),
            BuiltinFunction::IfElse => match self.if_else(use_provider, args) {
                // If successful, IfElse doesn't return a value, but diverges
                // the control flow.
                Ok(()) => return,
                Err(reason) => Err(reason),
            },
            BuiltinFunction::IntAdd => self.int_add(args),
            BuiltinFunction::IntBitLength => self.int_bit_length(args),
            BuiltinFunction::IntBitwiseAnd => self.int_bitwise_and(args),
            BuiltinFunction::IntBitwiseOr => self.int_bitwise_or(args),
            BuiltinFunction::IntBitwiseXor => self.int_bitwise_xor(args),
            BuiltinFunction::IntCompareTo => self.int_compare_to(args),
            BuiltinFunction::IntDivideTruncating => self.int_divide_truncating(args),
            BuiltinFunction::IntModulo => self.int_modulo(args),
            BuiltinFunction::IntMultiply => self.int_multiply(args),
            BuiltinFunction::IntParse => self.int_parse(args),
            BuiltinFunction::IntRemainder => self.int_remainder(args),
            BuiltinFunction::IntShiftLeft => self.int_shift_left(args),
            BuiltinFunction::IntShiftRight => self.int_shift_right(args),
            BuiltinFunction::IntSubtract => self.int_subtract(args),
            BuiltinFunction::Print => self.print(args),
            BuiltinFunction::StructGet => self.struct_get(args),
            BuiltinFunction::StructGetKeys => self.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.struct_has_key(args),
            BuiltinFunction::TextCharacters => self.text_characters(args),
            BuiltinFunction::TextConcatenate => self.text_concatenate(args),
            BuiltinFunction::TextContains => self.text_contains(args),
            BuiltinFunction::TextEndsWith => self.text_ends_with(args),
            BuiltinFunction::TextGetRange => self.text_get_range(args),
            BuiltinFunction::TextIsEmpty => self.text_is_empty(args),
            BuiltinFunction::TextLength => self.text_length(args),
            BuiltinFunction::TextStartsWith => self.text_starts_with(args),
            BuiltinFunction::TextTrimEnd => self.text_trim_end(args),
            BuiltinFunction::TextTrimStart => self.text_trim_start(args),
            BuiltinFunction::TypeOf => self.type_of(args),
        };
        let return_value = match return_value_or_panic_reason {
            Ok(value) => value,
            Err(reason) => self.panic(reason),
        };

        let return_object = self.heap.import(return_value);
        self.data_stack.push(return_object);
    }

    fn equals(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [a, b], { Ok((a == b).into()) })
    }

    fn function_run<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: Vec<Value>,
    ) -> Result<(), String> {
        destructure!(
            args,
            [Value::Closure(Closure {
                captured,
                num_args,
                body
            })],
            {
                if *num_args > 0 {
                    return Err(format!("`functionRun` expects a closure without arguments as the body, but got one with {num_args} arguments."));
                }
                let closure_object = self.heap.import(Value::Closure(Closure {
                    captured: captured.to_owned(),
                    num_args: *num_args,
                    body: body.to_owned(),
                }));
                log::debug!(
                    "`functionRun` executing the closure: {:?}",
                    self.heap.export_without_dropping(closure_object)
                );
                self.data_stack.push(closure_object);
                self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
                Ok(())
            }
        )
    }

    fn get_argument_count(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Closure(Closure { num_args, .. })], {
            Ok((*num_args).into())
        })
    }

    fn if_else<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: Vec<Value>,
    ) -> Result<(), String> {
        destructure!(
            args,
            [
                Value::Symbol(condition),
                Value::Closure(then_closure),
                Value::Closure(else_closure)
            ],
            {
                if then_closure.num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the then, but got one with {} arguments.", then_closure.num_args));
                }
                if else_closure.num_args > 0 {
                    return Err(format!("IfElse expects a closure without arguments as the else, but got one with {} arguments.", else_closure.num_args));
                }
                let condition = match condition.as_str() {
                    "True" => true,
                    "False" => false,
                    _ => {
                        return Err(format!(
                            "IfElse expected True or False as a condition, but got {condition}.",
                        ));
                    }
                };

                let closure_object = self.heap.import(if condition {
                    Value::Closure(then_closure.clone())
                } else {
                    Value::Closure(else_closure.clone())
                });
                log::debug!(
                    "IfElse executing the closure: {:?}",
                    self.heap.export_without_dropping(closure_object)
                );
                self.data_stack.push(closure_object);
                self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
                Ok(())
            }
        )
    }

    fn int_add(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(summand_a), Value::Int(summand_b)], {
            Ok((summand_a + summand_b).into())
        })
    }
    fn int_bit_length(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value)], {
            Ok(BigInt::from(value.bits()).into())
        })
    }
    fn int_bitwise_and(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value_a), Value::Int(value_b)], {
            Ok((value_a & value_b).into())
        })
    }
    fn int_bitwise_or(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value_a), Value::Int(value_b)], {
            Ok((value_a | value_b).into())
        })
    }
    fn int_bitwise_xor(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value_a), Value::Int(value_b)], {
            Ok((value_a ^ value_b).into())
        })
    }
    fn int_compare_to(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value_a), Value::Int(value_b)], {
            let result = match value_a.cmp(value_b) {
                Ordering::Less => "Less".to_string(),
                Ordering::Equal => "Equal".to_string(),
                Ordering::Greater => "Greater".to_string(),
            };
            Ok(Value::Symbol(result))
        })
    }
    fn int_divide_truncating(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend / divisor).into())
        })
    }
    fn int_modulo(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend.mod_floor(divisor)).into())
        })
    }
    fn int_multiply(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(factor_a), Value::Int(factor_b)], {
            Ok((factor_a * factor_b).into())
        })
    }
    fn int_parse(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], {
            Ok(BigInt::from_str(text).map_err(|it| format!("{it}")).into())
        })
    }
    fn int_remainder(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend % divisor).into())
        })
    }
    fn int_shift_left(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value), Value::Int(amount)], {
            let amount = amount.to_u128().unwrap();
            Ok((value << amount).into())
        })
    }
    fn int_shift_right(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(value), Value::Int(amount)], {
            let value = value.to_biguint().unwrap();
            let amount = amount.to_u128().unwrap();
            Ok((value >> amount).to_bigint().unwrap().into())
        })
    }
    fn int_subtract(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Int(minuend), Value::Int(subtrahend)], {
            Ok((minuend - subtrahend).into())
        })
    }

    fn print(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(message)], {
            log::info!("{message:?}");
            Ok(Value::nothing())
        })
    }

    fn struct_get(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_), key], {
            match struct_.get(key) {
                Some(value) => Ok(value.clone()),
                None => Err(format!("Struct does not contain key {key:?}.")),
            }
        })
    }

    fn struct_get_keys(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_)], {
            Ok(Value::list(struct_.keys().cloned().collect()))
        })
    }

    fn struct_has_key(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Struct(struct_), key], {
            Ok((struct_.contains_key(key)).into())
        })
    }

    fn text_characters(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], {
            Ok(Value::list(
                text.graphemes(true)
                    .map(|it| it.to_string().into())
                    .collect(),
            ))
        })
    }
    fn text_concatenate(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(value_a), Value::Text(value_b)], {
            Ok(format!("{value_a}{value_b}").into())
        })
    }
    fn text_contains(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text), Value::Text(pattern)], {
            Ok(text.contains(pattern).into())
        })
    }
    fn text_ends_with(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text), Value::Text(suffix)], {
            Ok(text.ends_with(suffix).into())
        })
    }
    fn text_get_range(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(
            args,
            [
                Value::Text(text),
                Value::Int(start_inclusive),
                Value::Int(end_exclusive)
            ],
            {
                let start_inclusive = start_inclusive.to_usize().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let end_exclusive = end_exclusive.to_usize().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let text = text
                    .graphemes(true)
                    .skip(start_inclusive)
                    .take(end_exclusive - start_inclusive)
                    .collect::<String>()
                    .into();
                Ok(text)
            }
        )
    }
    fn text_is_empty(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], { Ok(text.is_empty().into()) })
    }
    fn text_length(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], {
            Ok(text.graphemes(true).count().into())
        })
    }
    fn text_starts_with(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text), Value::Text(prefix)], {
            Ok(text.starts_with(prefix).into())
        })
    }
    fn text_trim_end(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], {
            Ok(text.trim_end().to_string().into())
        })
    }
    fn text_trim_start(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [Value::Text(text)], {
            Ok(text.trim_start().to_string().into())
        })
    }

    fn type_of(&mut self, args: Vec<Value>) -> Result<Value, String> {
        destructure!(args, [value], {
            Ok(Value::Symbol(
                match &value {
                    Value::Int(_) => "Int",
                    Value::Text(_) => "Text",
                    Value::Symbol(_) => "Symbol",
                    Value::Struct(_) => "Struct",
                    Value::Closure { .. } => "Function",
                    Value::Builtin { .. } => "Builtin",
                }
                .to_owned(),
            ))
        })
    }
}

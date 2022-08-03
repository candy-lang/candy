use std::{cmp::Ordering, collections::HashMap, str::FromStr};

use super::{
    heap::{ObjectData, ObjectPointer},
    use_provider::UseProvider,
    value::Value,
    Vm,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use num_bigint::{BigInt, ToBigInt};
use num_integer::Integer;
use num_traits::ToPrimitive;
use unicode_segmentation::UnicodeSegmentation;

macro_rules! export_and_destructure {
    ($vm:expr, $args:expr, $enum:pat, $body:block) => {{
        let args = $vm.heap.export_all($args);
        if let $enum = &args[..] {
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

        let return_value_or_panic_reason = match &builtin_function {
            BuiltinFunction::Equals => self.equals(args),
            BuiltinFunction::FunctionRun => match self.function_run(use_provider, args) {
                // If successful, `functionRun` doesn't return a value, but
                // diverges the control flow.
                Ok(()) => return,
                Err(reason) => Err(reason),
            },
            BuiltinFunction::GetArgumentCount => self.get_argument_count(args),
            BuiltinFunction::IfElse => match self.if_else(use_provider, args) {
                // If successful, `ifElse` doesn't return a value, but diverges
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

    fn equals(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [a, b], { Ok((a == b).into()) })
    }

    fn function_run<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: &[ObjectPointer],
    ) -> Result<(), String> {
        let closure_address = args.should_be_one_argument()?;
        let closure = self.heap.get(closure_address).data.should_be_a_closure()?;

        if closure.num_args > 0 {
            return Err(format!("`functionRun` expects a closure without arguments as the body, but got one with {} arguments.", closure.num_args));
        }
        log::debug!(
            "`functionRun` executing the closure: {:?}",
            self.heap.export_without_dropping(closure_address),
        );
        self.data_stack.push(closure_address);
        self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
        Ok(())
    }

    fn get_argument_count(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        let closure_address = args.should_be_one_argument()?;
        let closure = self.heap.get(closure_address).data.should_be_a_closure()?;
        self.heap.drop(closure_address);
        Ok(closure.num_args.into())
    }

    fn if_else<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: &[ObjectPointer],
    ) -> Result<(), String> {
        let (condition_address, then_address, else_address) = args.should_be_three_arguments()?;
        let condition = self.heap.export(condition_address).should_be_a_symbol()?;
        let then_closure = self.heap.get(then_address).data.should_be_a_closure()?;
        let else_closure = self.heap.get(else_address).data.should_be_a_closure()?;

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

        if condition {
            self.data_stack.push(then_address);
            self.heap.drop(else_address);
        } else {
            self.heap.drop(then_address);
            self.data_stack.push(else_address);
        }
        self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
        Ok(())
    }

    fn int_add(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(a), Value::Int(b)], {
            Ok((a + b).into())
        })
    }
    fn int_bit_length(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        let address = args.should_be_one_argument()?;
        let value = self.heap.export(address).should_be_an_int()?;
        Ok(BigInt::from(value.bits()).into())
    }
    fn int_bitwise_and(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(a), Value::Int(b)], {
            Ok((a & b).into())
        })
    }
    fn int_bitwise_or(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(a), Value::Int(b)], {
            Ok((a | b).into())
        })
    }
    fn int_bitwise_xor(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(a), Value::Int(b)], {
            Ok((a ^ b).into())
        })
    }
    fn int_compare_to(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(a), Value::Int(b)], {
            let result = match a.cmp(b) {
                Ordering::Less => "Less".to_string(),
                Ordering::Equal => "Equal".to_string(),
                Ordering::Greater => "Greater".to_string(),
            };
            Ok(Value::Symbol(result))
        })
    }
    fn int_divide_truncating(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend / divisor).into())
        })
    }
    fn int_modulo(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend.mod_floor(divisor)).into())
        })
    }
    fn int_multiply(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(factor_a), Value::Int(factor_b)], {
            Ok((factor_a * factor_b).into())
        })
    }
    fn int_parse(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(BigInt::from_str(text).map_err(|it| format!("{it}")).into())
        })
    }
    fn int_remainder(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(dividend), Value::Int(divisor)], {
            Ok((dividend % divisor).into())
        })
    }
    fn int_shift_left(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(value), Value::Int(amount)], {
            let amount = amount.to_u128().unwrap();
            Ok((value << amount).into())
        })
    }
    fn int_shift_right(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(value), Value::Int(amount)], {
            let value = value.to_biguint().unwrap();
            let amount = amount.to_u128().unwrap();
            Ok((value >> amount).to_bigint().unwrap().into())
        })
    }
    fn int_subtract(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Int(minuend), Value::Int(subtrahend)], {
            Ok((minuend - subtrahend).into())
        })
    }

    fn print(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(message)], {
            log::info!("{message:?}");
            Ok(Value::nothing())
        })
    }

    fn struct_get(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        let (struct_address, key_address) = args.should_be_two_arguments()?;
        let struct_ = self.heap.get(struct_address).data.should_be_a_struct()?;
        let key_to_find = self.heap.export(key_address);

        for (key, value) in struct_ {
            let key = self.heap.export_without_dropping(key);
            if key == key_to_find {
                let value = self.heap.export_without_dropping(value);
                self.heap.drop(struct_address);
                return Ok(value);
            }
        }
        Err(format!("Struct does not contain key {key_to_find:?}."))
    }

    fn struct_get_keys(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Struct(struct_)], {
            Ok(Value::list(struct_.keys().cloned().collect()))
        })
    }

    fn struct_has_key(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Struct(struct_), key], {
            Ok((struct_.contains_key(key)).into())
        })
    }

    fn text_characters(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(Value::list(
                text.graphemes(true)
                    .map(|it| it.to_string().into())
                    .collect(),
            ))
        })
    }
    fn text_concatenate(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(value_a), Value::Text(value_b)], {
            Ok(format!("{value_a}{value_b}").into())
        })
    }
    fn text_contains(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text), Value::Text(pattern)], {
            Ok(text.contains(pattern).into())
        })
    }
    fn text_ends_with(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text), Value::Text(suffix)], {
            Ok(text.ends_with(suffix).into())
        })
    }
    fn text_get_range(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(
            self,
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
    fn text_is_empty(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(text.is_empty().into())
        })
    }
    fn text_length(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(text.graphemes(true).count().into())
        })
    }
    fn text_starts_with(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text), Value::Text(prefix)], {
            Ok(text.starts_with(prefix).into())
        })
    }
    fn text_trim_end(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(text.trim_end().to_string().into())
        })
    }
    fn text_trim_start(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        export_and_destructure!(self, args, [Value::Text(text)], {
            Ok(text.trim_start().to_string().into())
        })
    }

    fn type_of(&mut self, args: &[ObjectPointer]) -> Result<Value, String> {
        let address = args.should_be_one_argument()?;
        let symbol = match self.heap.get(address).data {
            ObjectData::Int(_) => "Int",
            ObjectData::Text(_) => "Text",
            ObjectData::Symbol(_) => "Symbol",
            ObjectData::Struct(_) => "Struct",
            ObjectData::Closure { .. } => "Function",
            ObjectData::Builtin(_) => "Builtin",
        };
        self.heap.drop(address);
        Ok(Value::Symbol(symbol.to_string()))
    }
}

trait ShouldBeNArguments {
    fn should_be_one_argument(&self) -> Result<ObjectPointer, String>;
    fn should_be_two_arguments(&self) -> Result<(ObjectPointer, ObjectPointer), String>;
    fn should_be_three_arguments(
        &self,
    ) -> Result<(ObjectPointer, ObjectPointer, ObjectPointer), String>;
}
impl ShouldBeNArguments for [ObjectPointer] {
    fn should_be_one_argument(&self) -> Result<ObjectPointer, String> {
        if let [a] = self {
            Ok(*a)
        } else {
            Err(format!(
                "a builtin function expected 1 argument, but got {}",
                self.len()
            ))
        }
    }
    fn should_be_two_arguments(&self) -> Result<(ObjectPointer, ObjectPointer), String> {
        if let [a, b] = self {
            Ok((*a, *b))
        } else {
            Err(format!(
                "a builtin function expected 2 arguments, but got {}",
                self.len()
            ))
        }
    }
    fn should_be_three_arguments(
        &self,
    ) -> Result<(ObjectPointer, ObjectPointer, ObjectPointer), String> {
        if let [a, b, c] = self {
            Ok((*a, *b, *c))
        } else {
            Err(format!(
                "a builtin function expected 3 arguments, but got {}",
                self.len()
            ))
        }
    }
}
trait ValueShouldBeOfKind {
    fn should_be_an_int(self) -> Result<BigInt, String>;
    fn should_be_a_symbol(self) -> Result<String, String>;
}
impl ValueShouldBeOfKind for Value {
    fn should_be_an_int(self) -> Result<BigInt, String> {
        if let Value::Int(int) = self {
            Ok(int)
        } else {
            Err("a builtin function expected an int".to_string())
        }
    }
    fn should_be_a_symbol(self) -> Result<String, String> {
        if let Value::Symbol(symbol) = self {
            Ok(symbol)
        } else {
            Err("a builtin function expected a symbol".to_string())
        }
    }
}
trait ObjectDataShouldBeOfKind {
    fn should_be_an_int(&self) -> Result<BigInt, String>;
    fn should_be_a_symbol(&self) -> Result<String, String>;
    fn should_be_a_struct(&self) -> Result<HashMap<ObjectPointer, ObjectPointer>, String>;
    fn should_be_a_closure(&self) -> Result<Closure, String>;
}
impl ObjectDataShouldBeOfKind for ObjectData {
    fn should_be_an_int(&self) -> Result<BigInt, String> {
        if let ObjectData::Int(int) = self {
            Ok(int.clone())
        } else {
            Err("a builtin function expected an int".to_string())
        }
    }
    fn should_be_a_symbol(&self) -> Result<String, String> {
        if let ObjectData::Symbol(symbol) = self {
            Ok(symbol.to_string())
        } else {
            Err("a builtin function expected a symbol".to_string())
        }
    }
    fn should_be_a_struct(&self) -> Result<HashMap<ObjectPointer, ObjectPointer>, String> {
        if let ObjectData::Struct(fields) = self {
            Ok(fields.clone())
        } else {
            Err("a builtin function expected a struct".to_string())
        }
    }
    fn should_be_a_closure(&self) -> Result<Closure, String> {
        if let ObjectData::Closure { num_args, .. } = self {
            Ok(Closure {
                num_args: *num_args,
            })
        } else {
            Err("a builtin function expected a closure".to_string())
        }
    }
}
struct Closure {
    num_args: usize,
}

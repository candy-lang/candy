use super::{
    heap::{Closure, Data, Int, Pointer, Struct, Symbol, Text},
    use_provider::UseProvider,
    Vm,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::ToPrimitive;
use std::{ops::Deref, str::FromStr};
use unicode_segmentation::UnicodeSegmentation;

macro_rules! unpack_and_later_drop {
    ( $heap:expr, $args:expr, ($( $arg:ident: $type:ty ),+), $body:block ) => {
        {
            let ( $( $arg, )+ ) = if let [$( $arg, )+] = $args {
                ( $( *$arg, )+ )
            } else {
                return Err(
                    "a builtin function was called with the wrong number of arguments".to_string(),
                );
            };
            let ( $( $arg, )+ ): ( $( UnpackedData<$type>, )+ ) = ( $(
                UnpackedData {
                    address: $arg,
                    data: $heap.get($arg).data.clone().try_into()?,
                },
            )+ );

            let result = $body;
            $( $heap.drop($arg.address); )+
            result
        }
    };
}

type BuiltinResult = Result<Pointer, String>;
type DivergingBuiltinResult = Result<(), String>;

impl Vm {
    pub(super) fn run_builtin_function<U: UseProvider>(
        &mut self,
        use_provider: &U,
        builtin_function: &BuiltinFunction,
        args: &[Pointer],
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
        match return_value_or_panic_reason {
            Ok(return_value) => self.data_stack.push(return_value),
            Err(reason) => self.panic(reason),
        }
    }

    fn equals(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Any, b: Any), {
            let is_equal = a.equals(&self.heap, &*b);
            Ok(self.heap.create_bool(is_equal))
        })
    }

    fn function_run<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: &[Pointer],
    ) -> DivergingBuiltinResult {
        unpack_and_later_drop!(self.heap, args, (closure: Closure), {
            closure.should_take_no_arguments()?;
            log::debug!(
                "`functionRun` executing the closure: {}",
                closure.address.format(&self.heap),
            );
            self.heap.dup(closure.address);
            self.data_stack.push(closure.address);
            self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
            Ok(())
        })
    }

    fn get_argument_count(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (closure: Closure), {
            Ok(self.heap.create_int(closure.num_args.into()))
        })
    }

    fn if_else<U: UseProvider>(
        &mut self,
        use_provider: &U,
        args: &[Pointer],
    ) -> DivergingBuiltinResult {
        unpack_and_later_drop!(
            self.heap,
            args,
            (condition: bool, then: Closure, else_: Closure),
            {
                if *condition {
                    self.heap.dup(then.address);
                    self.data_stack.push(then.address);
                } else {
                    self.heap.dup(else_.address);
                    self.data_stack.push(else_.address);
                }
                self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
                Ok(())
            }
        )
    }

    fn int_add(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int, b: Int), {
            Ok(self
                .heap
                .create_int((a.value.clone() + b.value.clone()).into()))
        })
    }
    fn int_bit_length(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int), {
            Ok(self.heap.create_int(a.value.bits().into()))
        })
    }
    fn int_bitwise_and(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int, b: Int), {
            Ok(self
                .heap
                .create_int((a.value.clone() & b.value.clone()).into()))
        })
    }
    fn int_bitwise_or(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int, b: Int), {
            Ok(self
                .heap
                .create_int((a.value.clone() | b.value.clone()).into()))
        })
    }
    fn int_bitwise_xor(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int, b: Int), {
            Ok(self
                .heap
                .create_int((a.value.clone() ^ b.value.clone()).into()))
        })
    }
    fn int_compare_to(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Int, b: Int), {
            Ok(self.heap.create_ordering(a.value.cmp(&b.value)))
        })
    }
    fn int_divide_truncating(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (dividend: Int, divisor: Int), {
            Ok(self
                .heap
                .create_int((dividend.value.clone() / divisor.value.clone()).into()))
        })
    }
    fn int_modulo(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (dividend: Int, divisor: Int), {
            Ok(self
                .heap
                .create_int(dividend.value.clone().mod_floor(&divisor.value).into()))
        })
    }
    fn int_multiply(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (factor_a: Int, factor_b: Int), {
            Ok(self
                .heap
                .create_int((factor_a.value.clone() * factor_b.value.clone()).into()))
        })
    }
    fn int_parse(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            let result = match BigInt::from_str(&text.value) {
                Ok(int) => Ok(self.heap.create_int(int)),
                Err(err) => Err(self.heap.create_text(format!("{err}"))),
            };
            Ok(self.heap.create_result(result))
        })
    }
    fn int_remainder(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (dividend: Int, divisor: Int), {
            Ok(self
                .heap
                .create_int((dividend.value.clone() % divisor.value.clone()).into()))
        })
    }
    fn int_shift_left(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (value: Int, amount: Int), {
            let amount = amount.value.to_u128().unwrap();
            Ok(self.heap.create_int((value.value.clone() << amount).into()))
        })
    }
    fn int_shift_right(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (value: Int, amount: Int), {
            let value = value.value.to_biguint().unwrap();
            let amount = amount.value.to_u128().unwrap();
            Ok(self.heap.create_int((value >> amount).into()))
        })
    }
    fn int_subtract(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (minuend: Int, subtrahend: Int), {
            Ok(self
                .heap
                .create_int((minuend.value.clone() - subtrahend.value.clone()).into()))
        })
    }

    fn print(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (message: Text), {
            log::info!("{:?}", message.value);
            Ok(self.heap.create_nothing())
        })
    }

    fn struct_get(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (struct_: Struct, key: Any), {
            match struct_.get(&self.heap, key.address) {
                Some(value) => {
                    self.heap.dup(value);
                    Ok(value)
                }
                None => Err(format!(
                    "Struct does not contain key {}.",
                    key.format(&self.heap)
                )),
            }
        })
    }
    fn struct_get_keys(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (struct_: Struct), {
            Ok(self
                .heap
                .create_list(struct_.iter().map(|(key, _)| key.clone()).collect()))
        })
    }
    fn struct_has_key(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (struct_: Struct, key: Any), {
            let has_key = struct_.get(&self.heap, key.address).is_some();
            Ok(self.heap.create_bool(has_key))
        })
    }

    fn text_characters(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            let mut character_addresses = vec![];
            for c in text.value.graphemes(true) {
                character_addresses.push(self.heap.create_text(c.to_string()));
            }
            Ok(self.heap.create_list(character_addresses))
        })
    }
    fn text_concatenate(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (a: Text, b: Text), {
            Ok(self.heap.create_text(format!("{}{}", a.value, b.value)))
        })
    }
    fn text_contains(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text, pattern: Text), {
            Ok(self.heap.create_bool(text.value.contains(&pattern.value)))
        })
    }
    fn text_ends_with(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text, suffix: Text), {
            Ok(self.heap.create_bool(text.value.ends_with(&suffix.value)))
        })
    }
    fn text_get_range(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(
            self.heap,
            args,
            (text: Text, start_inclusive: Int, end_exclusive: Int),
            {
                let start_inclusive = start_inclusive.value.to_usize().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let end_exclusive = end_exclusive.value.to_usize().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let text = text
                    .value
                    .graphemes(true)
                    .skip(start_inclusive)
                    .take(end_exclusive - start_inclusive)
                    .collect();
                Ok(self.heap.create_text(text))
            }
        )
    }
    fn text_is_empty(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            Ok(self.heap.create_bool(text.value.is_empty()))
        })
    }
    fn text_length(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            let length = text.value.graphemes(true).count().into();
            Ok(self.heap.create_int(length))
        })
    }
    fn text_starts_with(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text, prefix: Text), {
            Ok(self.heap.create_bool(text.value.starts_with(&prefix.value)))
        })
    }
    fn text_trim_end(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            Ok(self.heap.create_text(text.value.trim_end().to_string()))
        })
    }
    fn text_trim_start(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (text: Text), {
            Ok(self.heap.create_text(text.value.trim_start().to_string()))
        })
    }

    fn type_of(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self.heap, args, (value: Any), {
            let symbol = match **value {
                Data::Int(_) => "Int",
                Data::Text(_) => "Text",
                Data::Symbol(_) => "Symbol",
                Data::Struct(_) => "Struct",
                Data::Closure(_) => "Function",
                Data::Builtin(_) => "Builtin",
            };
            Ok(self.heap.create_symbol(symbol.to_string()))
        })
    }
}

trait ClosureShouldHaveNoArguments {
    fn should_take_no_arguments(&self) -> Result<(), String>;
}
impl ClosureShouldHaveNoArguments for Closure {
    fn should_take_no_arguments(&self) -> Result<(), String> {
        match self.num_args {
            0 => Ok(()),
            n => Err(format!("a builtin function expected a function without arguments, but got one that takes {n} arguments")),
        }
    }
}

struct UnpackedData<T> {
    address: Pointer,
    data: T,
}
impl<T> Deref for UnpackedData<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

struct Any {
    data: Data,
}
impl Deref for Any {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl TryInto<Any> for Data {
    type Error = String;

    fn try_into(self) -> Result<Any, Self::Error> {
        Ok(Any { data: self })
    }
}
impl TryInto<Int> for Data {
    type Error = String;

    fn try_into(self) -> Result<Int, Self::Error> {
        match self {
            Data::Int(int) => Ok(int),
            _ => Err("a builtin function expected an int".to_string()),
        }
    }
}
impl TryInto<Text> for Data {
    type Error = String;

    fn try_into(self) -> Result<Text, Self::Error> {
        match self {
            Data::Text(text) => Ok(text),
            _ => Err("a builtin function expected a text".to_string()),
        }
    }
}
impl TryInto<Symbol> for Data {
    type Error = String;

    fn try_into(self) -> Result<Symbol, Self::Error> {
        match self {
            Data::Symbol(symbol) => Ok(symbol),
            _ => Err("a builtin function expected a symbol".to_string()),
        }
    }
}
impl TryInto<Struct> for Data {
    type Error = String;

    fn try_into(self) -> Result<Struct, Self::Error> {
        match self {
            Data::Struct(struct_) => Ok(struct_),
            _ => Err("a builtin function expected a struct".to_string()),
        }
    }
}
impl TryInto<Closure> for Data {
    type Error = String;

    fn try_into(self) -> Result<Closure, Self::Error> {
        match self {
            Data::Closure(closure) => Ok(closure),
            _ => Err("a builtin function expected a function".to_string()),
        }
    }
}
impl TryInto<bool> for Data {
    type Error = String;

    fn try_into(self) -> Result<bool, Self::Error> {
        let symbol: Symbol = self.try_into()?;
        match symbol.value.as_str() {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err("a builtin function expected `True` or `False`".to_string()),
        }
    }
}

use super::{
    channel::{Capacity, Packet},
    fiber::{Fiber, Status},
    heap::{ChannelId, Closure, Data, Int, Pointer, ReceivePort, SendPort, Struct, Symbol, Text},
    use_provider::UseProvider,
    Heap,
};
use crate::{builtin_functions::BuiltinFunction, compiler::lir::Instruction};
use itertools::Itertools;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::ToPrimitive;
use std::{ops::Deref, str::FromStr};
use tracing::{info, span, Level};
use unicode_segmentation::UnicodeSegmentation;

impl Fiber {
    pub(super) fn run_builtin_function<U: UseProvider>(
        &mut self,
        use_provider: &U,
        builtin_function: &BuiltinFunction,
        args: &[Pointer],
    ) {
        let result = span!(Level::TRACE, "Running builtin").in_scope(|| match &builtin_function {
            BuiltinFunction::ChannelCreate => self.heap.channel_create(args),
            BuiltinFunction::ChannelSend => self.heap.channel_send(args),
            BuiltinFunction::ChannelReceive => self.heap.channel_receive(args),
            BuiltinFunction::Equals => self.heap.equals(args),
            BuiltinFunction::FunctionRun => self.heap.function_run(args),
            BuiltinFunction::GetArgumentCount => self.heap.get_argument_count(args),
            BuiltinFunction::IfElse => self.heap.if_else(args),
            BuiltinFunction::IntAdd => self.heap.int_add(args),
            BuiltinFunction::IntBitLength => self.heap.int_bit_length(args),
            BuiltinFunction::IntBitwiseAnd => self.heap.int_bitwise_and(args),
            BuiltinFunction::IntBitwiseOr => self.heap.int_bitwise_or(args),
            BuiltinFunction::IntBitwiseXor => self.heap.int_bitwise_xor(args),
            BuiltinFunction::IntCompareTo => self.heap.int_compare_to(args),
            BuiltinFunction::IntDivideTruncating => self.heap.int_divide_truncating(args),
            BuiltinFunction::IntModulo => self.heap.int_modulo(args),
            BuiltinFunction::IntMultiply => self.heap.int_multiply(args),
            BuiltinFunction::IntParse => self.heap.int_parse(args),
            BuiltinFunction::IntRemainder => self.heap.int_remainder(args),
            BuiltinFunction::IntShiftLeft => self.heap.int_shift_left(args),
            BuiltinFunction::IntShiftRight => self.heap.int_shift_right(args),
            BuiltinFunction::IntSubtract => self.heap.int_subtract(args),
            BuiltinFunction::Parallel => self.heap.parallel(args),
            BuiltinFunction::Print => self.heap.print(args),
            BuiltinFunction::StructGet => self.heap.struct_get(args),
            BuiltinFunction::StructGetKeys => self.heap.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.heap.struct_has_key(args),
            BuiltinFunction::TextCharacters => self.heap.text_characters(args),
            BuiltinFunction::TextConcatenate => self.heap.text_concatenate(args),
            BuiltinFunction::TextContains => self.heap.text_contains(args),
            BuiltinFunction::TextEndsWith => self.heap.text_ends_with(args),
            BuiltinFunction::TextGetRange => self.heap.text_get_range(args),
            BuiltinFunction::TextIsEmpty => self.heap.text_is_empty(args),
            BuiltinFunction::TextLength => self.heap.text_length(args),
            BuiltinFunction::TextStartsWith => self.heap.text_starts_with(args),
            BuiltinFunction::TextTrimEnd => self.heap.text_trim_end(args),
            BuiltinFunction::TextTrimStart => self.heap.text_trim_start(args),
            BuiltinFunction::TypeOf => self.heap.type_of(args),
        });
        match result {
            Ok(Return(value)) => self.data_stack.push(value),
            Ok(DivergeControlFlow { closure }) => {
                self.data_stack.push(closure);
                self.run_instruction(use_provider, Instruction::Call { num_args: 0 });
            }
            Ok(CreateChannel { capacity }) => self.status = Status::CreatingChannel { capacity },
            Ok(Send { channel, packet }) => self.status = Status::Sending { channel, packet },
            Ok(Receive { channel }) => self.status = Status::Receiving { channel },
            Ok(Parallel { body }) => self.status = Status::InParallelScope { body },
            Err(reason) => self.panic(reason),
        }
    }
}

type BuiltinResult = Result<SuccessfulBehavior, String>;
enum SuccessfulBehavior {
    Return(Pointer),
    DivergeControlFlow { closure: Pointer },
    CreateChannel { capacity: Capacity },
    Send { channel: ChannelId, packet: Packet },
    Receive { channel: ChannelId },
    Parallel { body: Pointer },
}
use SuccessfulBehavior::*;

impl From<SuccessfulBehavior> for BuiltinResult {
    fn from(ok: SuccessfulBehavior) -> Self {
        Ok(ok)
    }
}

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
            result.into()
        }
    };
}

impl Heap {
    fn channel_create(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (capacity: Int), {
            CreateChannel {
                capacity: capacity.value.clone().try_into().expect(
                    "you tried to create a channel with a capacity bigger than the maximum usize",
                ),
            }
        })
    }

    fn channel_send(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (port: SendPort, packet: Any), {
            let mut heap = Heap::default();
            let value = self.clone_single_to_other_heap(&mut heap, packet.address);
            Send {
                channel: port.channel,
                packet: Packet { heap, value },
            }
        })
    }

    fn channel_receive(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (port: ReceivePort), {
            Receive {
                channel: port.channel,
            }
        })
    }

    fn equals(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Any, b: Any), {
            let is_equal = a.equals(self, &b);
            Return(self.create_bool(is_equal))
        })
    }

    fn function_run(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (closure: Closure), {
            closure.should_take_no_arguments()?;
            self.dup(closure.address);
            DivergeControlFlow {
                closure: closure.address,
            }
        })
    }

    fn get_argument_count(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (closure: Closure), {
            Return(self.create_int(closure.num_args.into()))
        })
    }

    fn if_else(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(
            self,
            args,
            (condition: bool, then: Closure, else_: Closure),
            {
                let closure_to_run = if *condition { &then } else { &else_ }.address;
                self.dup(closure_to_run);
                DivergeControlFlow {
                    closure: closure_to_run,
                }
            }
        )
    }

    fn int_add(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int, b: Int), {
            Return(self.create_int(a.value.clone() + b.value.clone()))
        })
    }
    fn int_bit_length(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int), {
            Return(self.create_int(a.value.bits().into()))
        })
    }
    fn int_bitwise_and(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int, b: Int), {
            Return(self.create_int(a.value.clone() & b.value.clone()))
        })
    }
    fn int_bitwise_or(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int, b: Int), {
            Return(self.create_int(a.value.clone() | b.value.clone()))
        })
    }
    fn int_bitwise_xor(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int, b: Int), {
            Return(self.create_int(a.value.clone() ^ b.value.clone()))
        })
    }
    fn int_compare_to(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Int, b: Int), {
            Return(self.create_ordering(a.value.cmp(&b.value)))
        })
    }
    fn int_divide_truncating(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (dividend: Int, divisor: Int), {
            Return(self.create_int(dividend.value.clone() / divisor.value.clone()))
        })
    }
    fn int_modulo(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (dividend: Int, divisor: Int), {
            Return(self.create_int(dividend.value.clone().mod_floor(&divisor.value)))
        })
    }
    fn int_multiply(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (factor_a: Int, factor_b: Int), {
            Return(self.create_int(factor_a.value.clone() * factor_b.value.clone()))
        })
    }
    fn int_parse(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            let result = match BigInt::from_str(&text.value) {
                Ok(int) => Ok(self.create_int(int)),
                Err(err) => Err(self.create_text(format!("{err}"))),
            };
            Return(self.create_result(result))
        })
    }
    fn int_remainder(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (dividend: Int, divisor: Int), {
            Return(self.create_int(dividend.value.clone() % divisor.value.clone()))
        })
    }
    fn int_shift_left(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (value: Int, amount: Int), {
            let amount = amount.value.to_u128().unwrap();
            Return(self.create_int(value.value.clone() << amount))
        })
    }
    fn int_shift_right(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (value: Int, amount: Int), {
            let value = value.value.to_biguint().unwrap();
            let amount = amount.value.to_u128().unwrap();
            Return(self.create_int((value >> amount).into()))
        })
    }
    fn int_subtract(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (minuend: Int, subtrahend: Int), {
            Return(self.create_int(minuend.value.clone() - subtrahend.value.clone()))
        })
    }

    fn parallel(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (body_taking_nursery: Closure), {
            self.dup(body_taking_nursery.address);
            Parallel {
                body: body_taking_nursery.address,
            }
        })
    }

    fn print(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (message: Text), {
            info!("{:?}", message.value);
            Return(self.create_nothing())
        })
    }

    fn struct_get(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (struct_: Struct, key: Any), {
            match struct_.get(self, key.address) {
                Some(value) => {
                    self.dup(value);
                    Ok(Return(value))
                }
                None => Err(format!("Struct does not contain key {}.", key.format(self))),
            }
        })
    }
    fn struct_get_keys(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (struct_: Struct), {
            Return(self.create_list(&struct_.iter().map(|(key, _)| key).collect_vec()))
        })
    }
    fn struct_has_key(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (struct_: Struct, key: Any), {
            let has_key = struct_.get(self, key.address).is_some();
            Return(self.create_bool(has_key))
        })
    }

    fn text_characters(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            let mut character_addresses = vec![];
            for c in text.value.graphemes(true) {
                character_addresses.push(self.create_text(c.to_string()));
            }
            Return(self.create_list(&character_addresses))
        })
    }
    fn text_concatenate(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (a: Text, b: Text), {
            Return(self.create_text(format!("{}{}", a.value, b.value)))
        })
    }
    fn text_contains(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text, pattern: Text), {
            Return(self.create_bool(text.value.contains(&pattern.value)))
        })
    }
    fn text_ends_with(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text, suffix: Text), {
            Return(self.create_bool(text.value.ends_with(&suffix.value)))
        })
    }
    fn text_get_range(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(
            self,
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
                Return(self.create_text(text))
            }
        )
    }
    fn text_is_empty(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            Return(self.create_bool(text.value.is_empty()))
        })
    }
    fn text_length(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            let length = text.value.graphemes(true).count().into();
            Return(self.create_int(length))
        })
    }
    fn text_starts_with(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text, prefix: Text), {
            Return(self.create_bool(text.value.starts_with(&prefix.value)))
        })
    }
    fn text_trim_end(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            Return(self.create_text(text.value.trim_end().to_string()))
        })
    }
    fn text_trim_start(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (text: Text), {
            Return(self.create_text(text.value.trim_start().to_string()))
        })
    }

    fn type_of(&mut self, args: &[Pointer]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, (value: Any), {
            let symbol = match **value {
                Data::Int(_) => "Int",
                Data::Text(_) => "Text",
                Data::Symbol(_) => "Symbol",
                Data::Struct(_) => "Struct",
                Data::Closure(_) => "Function",
                Data::Builtin(_) => "Builtin",
                Data::SendPort(_) => "SendPort",
                Data::ReceivePort(_) => "ReceivePort",
            };
            Return(self.create_symbol(symbol.to_string()))
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
impl TryInto<SendPort> for Data {
    type Error = String;

    fn try_into(self) -> Result<SendPort, Self::Error> {
        match self {
            Data::SendPort(port) => Ok(port),
            _ => Err("a builtin function expected a send port".to_string()),
        }
    }
}
impl TryInto<ReceivePort> for Data {
    type Error = String;

    fn try_into(self) -> Result<ReceivePort, Self::Error> {
        match self {
            Data::ReceivePort(port) => Ok(port),
            _ => Err("a builtin function expected a receive port".to_string()),
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

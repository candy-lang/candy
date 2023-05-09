use crate::{
    channel::ChannelId,
    channel::{Capacity, Packet},
    fiber::{Fiber, Panic, Status},
    heap::{
        Data, Function, Heap, HirId, InlineObject, Int, List, ReceivePort, SendPort, Struct, Tag,
        Text,
    },
    tracer::FiberTracer,
};
use candy_frontend::builtin_functions::BuiltinFunction;
use itertools::Itertools;
use num_bigint::BigInt;
use paste::paste;
use std::str::{self, FromStr};
use tracing::{info, span, Level};
use unicode_segmentation::UnicodeSegmentation;

impl<FT: FiberTracer> Fiber<FT> {
    pub(super) fn run_builtin_function(
        &mut self,
        builtin_function: BuiltinFunction,
        args: &[InlineObject],
        responsible: HirId,
    ) {
        let result = span!(Level::TRACE, "Running builtin").in_scope(|| match &builtin_function {
            BuiltinFunction::ChannelCreate => self.heap.channel_create(args),
            BuiltinFunction::ChannelSend => self.heap.channel_send(args),
            BuiltinFunction::ChannelReceive => self.heap.channel_receive(args),
            BuiltinFunction::Equals => self.heap.equals(args),
            BuiltinFunction::FunctionRun => self.heap.function_run(args, responsible),
            BuiltinFunction::GetArgumentCount => self.heap.get_argument_count(args),
            BuiltinFunction::IfElse => self.heap.if_else(args, responsible),
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
            BuiltinFunction::ListFilled => self.heap.list_filled(args),
            BuiltinFunction::ListGet => self.heap.list_get(args),
            BuiltinFunction::ListInsert => self.heap.list_insert(args),
            BuiltinFunction::ListLength => self.heap.list_length(args),
            BuiltinFunction::ListRemoveAt => self.heap.list_remove_at(args),
            BuiltinFunction::ListReplace => self.heap.list_replace(args),
            BuiltinFunction::Parallel => self.heap.parallel(args),
            BuiltinFunction::Print => self.heap.print(args),
            BuiltinFunction::StructGet => self.heap.struct_get(args),
            BuiltinFunction::StructGetKeys => self.heap.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.heap.struct_has_key(args),
            BuiltinFunction::TagGetValue => self.heap.tag_get_value(args),
            BuiltinFunction::TagHasValue => self.heap.tag_has_value(args),
            BuiltinFunction::TagWithoutValue => self.heap.tag_without_value(args),
            BuiltinFunction::TextCharacters => self.heap.text_characters(args),
            BuiltinFunction::TextConcatenate => self.heap.text_concatenate(args),
            BuiltinFunction::TextContains => self.heap.text_contains(args),
            BuiltinFunction::TextEndsWith => self.heap.text_ends_with(args),
            BuiltinFunction::TextFromUtf8 => self.heap.text_from_utf8(args),
            BuiltinFunction::TextGetRange => self.heap.text_get_range(args),
            BuiltinFunction::TextIsEmpty => self.heap.text_is_empty(args),
            BuiltinFunction::TextLength => self.heap.text_length(args),
            BuiltinFunction::TextStartsWith => self.heap.text_starts_with(args),
            BuiltinFunction::TextTrimEnd => self.heap.text_trim_end(args),
            BuiltinFunction::TextTrimStart => self.heap.text_trim_start(args),
            BuiltinFunction::ToDebugText => self.heap.to_debug_text(args),
            BuiltinFunction::Try => self.heap.try_(args),
            BuiltinFunction::TypeOf => self.heap.type_of(args),
        });
        match result {
            Ok(Return(value)) => self.data_stack.push(value),
            Ok(DivergeControlFlow {
                function,
                responsible,
            }) => self.call_function(function, &[], responsible),
            Ok(CreateChannel { capacity }) => self.status = Status::CreatingChannel { capacity },
            Ok(Send { channel, packet }) => self.status = Status::Sending { channel, packet },
            Ok(Receive { channel }) => self.status = Status::Receiving { channel },
            Ok(Parallel { body }) => self.status = Status::InParallelScope { body },
            Ok(Try { body }) => self.status = Status::InTry { body },
            Err(reason) => self.panic(Panic::new(reason, responsible.get().to_owned())),
        }
    }
}

type BuiltinResult = Result<SuccessfulBehavior, String>;
enum SuccessfulBehavior {
    Return(InlineObject),
    DivergeControlFlow {
        function: Function,
        responsible: HirId,
    },
    CreateChannel {
        capacity: Capacity,
    },
    Send {
        channel: ChannelId,
        packet: Packet,
    },
    Receive {
        channel: ChannelId,
    },
    Parallel {
        body: Function,
    },
    Try {
        body: Function,
    },
}
use derive_more::Deref;
use SuccessfulBehavior::*;

impl From<SuccessfulBehavior> for BuiltinResult {
    fn from(ok: SuccessfulBehavior) -> Self {
        Ok(ok)
    }
}

macro_rules! unpack {
    ( $heap:expr, $args:expr, |$( $arg:ident: $type:ty ),+| $body:block ) => {
        {
            let ( $( $arg, )+ ) = if let [$( $arg, )+] = $args {
                ( $( *$arg, )+ )
            } else {
                return Err(
                    "A builtin function was called with the wrong number of arguments.".to_string(),
                );
            };
            let ( $( $arg, )+ ): ( $( UnpackedData<$type>, )+ ) = ( $(
                UnpackedData {
                    object: $arg,
                    data: $arg.try_into()?,
                },
            )+ );

            $body.into()
        }
    };
}
macro_rules! unpack_and_later_drop {
    ( $heap:expr, $args:expr, |$( $arg:ident: $type:ty ),+| $body:block ) => {
        {
            let ( $( $arg, )+ ) = if let [$( $arg, )+] = $args {
                ( $( *$arg, )+ )
            } else {
                return Err(
                    "A builtin function was called with the wrong number of arguments.".to_string(),
                );
            };
            let ( $( $arg, )+ ): ( $( UnpackedData<$type>, )+ ) = ( $(
                UnpackedData {
                    object: $arg,
                    data: $arg.try_into()?,
                },
            )+ );

            // Structs are called `struct_`, so we sometimes generate
            // identifiers containing a double underscore.
            #[allow(non_snake_case)]
            $( let paste!([< $arg _object >]) = $arg.object; )+

            let result = $body;

            $( paste!([< $arg _object >]).drop($heap); )+
            result.into()
        }
    };
}

impl Heap {
    fn channel_create(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |capacity: Int| {
            match capacity.try_get() {
                Some(capacity) => CreateChannel { capacity },
                None => return Err("You tried to create a channel with a capacity that is either negative or bigger than the maximum usize.".to_string()),
            }
        })
    }

    fn channel_send(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |port: SendPort, packet: Any| {
            let mut heap = Heap::default();
            let object = packet.object.clone_to_heap(&mut heap);
            Send {
                channel: port.channel_id(),
                packet: Packet { heap, object },
            }
        })
    }

    fn channel_receive(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |port: ReceivePort| {
            Receive {
                channel: port.channel_id(),
            }
        })
    }

    fn equals(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Any, b: Any| {
            Return(Tag::create_bool(self, **a == **b).into())
        })
    }

    fn function_run(&mut self, args: &[InlineObject], responsible: HirId) -> BuiltinResult {
        unpack!(self, args, |function: Function| {
            function.should_take_no_arguments()?;
            DivergeControlFlow {
                function: *function,
                responsible,
            }
        })
    }

    fn get_argument_count(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |function: Function| {
            Return(Int::create(self, function.argument_count()).into())
        })
    }

    fn if_else(&mut self, args: &[InlineObject], responsible: HirId) -> BuiltinResult {
        unpack!(self, args, |condition: bool,
                             then: Function,
                             else_: Function| {
            let (run, dont_run) = if *condition {
                (then, else_)
            } else {
                (else_, then)
            };

            condition.object.drop(self);
            dont_run.object.drop(self);

            DivergeControlFlow {
                function: *run,
                responsible,
            }
        })
    }

    fn int_add(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.add(self, &b).into())
        })
    }
    fn int_bit_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int| { Return(a.bit_length(self).into()) })
    }
    fn int_bitwise_and(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_and(self, &b).into())
        })
    }
    fn int_bitwise_or(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_or(self, &b).into())
        })
    }
    fn int_bitwise_xor(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_xor(self, &b).into())
        })
    }
    fn int_compare_to(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.compare_to(self, &b).into())
        })
    }
    fn int_divide_truncating(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            if divisor.try_get::<u8>() == 0.into() {
                return Err("Can't divide by zero.".to_string());
            }
            Return(dividend.int_divide_truncating(self, &divisor).into())
        })
    }
    fn int_modulo(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            if divisor.try_get::<u8>() == 0.into() {
                return Err("Can't divide by zero.".to_string());
            }
            Return(dividend.modulo(self, &divisor).into())
        })
    }
    fn int_multiply(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |factor_a: Int, factor_b: Int| {
            Return(factor_a.multiply(self, &factor_b).into())
        })
    }
    fn int_parse(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            let result = match BigInt::from_str(text.get()) {
                Ok(int) => Ok(Int::create_from_bigint(self, int).into()),
                Err(err) => Err(Text::create(self, &err.to_string()).into()),
            };
            Return(Struct::create_result(self, result).into())
        })
    }
    fn int_remainder(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            if divisor.try_get::<u8>() == 0.into() {
                return Err("Can't divide by zero.".to_string());
            }
            Return(dividend.remainder(self, &divisor).into())
        })
    }
    fn int_shift_left(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Int, amount: Int| {
            Return(value.shift_left(self, &amount).into())
        })
    }
    fn int_shift_right(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Int, amount: Int| {
            Return(value.shift_right(self, &amount).into())
        })
    }
    fn int_subtract(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |minuend: Int, subtrahend: Int| {
            Return(minuend.subtract(self, &subtrahend).into())
        })
    }

    fn list_filled(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |length: Int, item: Any| {
            let length_usize = length.try_get().unwrap();
            length.object.drop(self);

            let item_object = item.object;
            if length_usize == 0 {
                item.object.drop(self);
            } else {
                item.object.dup_by(self, length_usize - 1);
            }

            Return(List::create(self, &vec![item_object; length_usize]).into())
        })
    }
    fn list_get(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |list: List, index: Int| {
            let index = index.try_get().unwrap();
            let item = list.get(index);
            item.dup(self);
            Return(item)
        })
    }
    fn list_insert(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |list: List, index: Int, item: Any| {
            let index_usize = index.try_get().unwrap();
            index.object.drop(self);

            let new_list = list.insert(self, index_usize, item.object).into();
            list.object.drop(self);
            Return(new_list)
        })
    }
    fn list_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |list: List| {
            Return(Int::create(self, list.len()).into())
        })
    }
    fn list_remove_at(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |list: List, index: Int| {
            Return(list.remove(self, index.try_get().unwrap()).into())
        })
    }
    fn list_replace(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |list: List, index: Int, new_item: Any| {
            let index_usize = index.try_get().unwrap();
            index.object.drop(self);

            list.get(index_usize).drop(self);

            let new_list = list.replace(self, index_usize, new_item.object).into();
            list.object.drop(self);
            Return(new_list)
        })
    }

    fn parallel(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |body_taking_nursery: Function| {
            if body_taking_nursery.argument_count() != 1 {
                return Err("`parallel` expects a function taking a nursery.".to_string());
            }
            Parallel {
                body: *body_taking_nursery,
            }
        })
    }

    fn print(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |message: Any| {
            info!("{}", message.object);
            Return(Tag::create_nothing(self).into())
        })
    }

    fn struct_get(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct, key: Any| {
            match struct_.get(key.object) {
                Some(value) => {
                    value.dup(self);
                    Ok(Return(value))
                }
                None => Err(format!(
                    "The struct does not contain the key {}.",
                    key.object,
                )),
            }
        })
    }
    fn struct_get_keys(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct| {
            Return(List::create(self, struct_.keys()).into())
        })
    }
    fn struct_has_key(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct, key: Any| {
            Return(Tag::create_bool(self, struct_.contains(key.object)).into())
        })
    }

    fn tag_get_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            tag.value()
                .map(|value| {
                    value.dup(self);
                    Return(value)
                })
                .ok_or_else(|| "The tag doesn't have a value.".to_string())
        })
    }
    fn tag_has_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            Return(Tag::create_bool(self, tag.value().is_some()).into())
        })
    }
    fn tag_without_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            Return(Tag::create(self, tag.symbol(), None).into())
        })
    }

    fn text_characters(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            let characters = text
                .get()
                .graphemes(true)
                .map(|it| Text::create(self, it).into())
                .collect_vec();
            Return(List::create(self, &characters).into())
        })
    }
    fn text_concatenate(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text_a: Text, text_b: Text| {
            Return(Text::create(self, &format!("{}{}", text_a.get(), text_b.get())).into())
        })
    }
    fn text_contains(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, pattern: Text| {
            Return(Tag::create_bool(self, text.get().contains(pattern.get())).into())
        })
    }
    fn text_ends_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, suffix: Text| {
            Return(Tag::create_bool(self, text.get().ends_with(suffix.get())).into())
        })
    }
    fn text_from_utf8(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |bytes: List| {
            let bytes: Vec<_> = bytes
                .items()
                .iter()
                .map(|&it| {
                    Int::try_from(it)
                        .ok()
                        .and_then(|it| it.try_get())
                        .ok_or_else(|| format!("Value is not a byte: {it}."))
                })
                .try_collect()?;
            let result = str::from_utf8(&bytes)
                .map(|it| Text::create(self, it).into())
                .map_err(|_| Text::create(self, "Invalid UTF-8.").into());
            Return(Struct::create_result(self, result).into())
        })
    }
    fn text_get_range(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(
            self,
            args,
            |text: Text, start_inclusive: Int, end_exclusive: Int| {
                let start_inclusive = start_inclusive.try_get().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let end_exclusive = end_exclusive.try_get::<usize>().expect(
                    "Tried to get a range from a text with an index that's too large for usize.",
                );
                let text: String = text
                    .get()
                    .graphemes(true)
                    .skip(start_inclusive)
                    .take(end_exclusive - start_inclusive)
                    .collect();
                Return(Text::create(self, &text).into())
            }
        )
    }
    fn text_is_empty(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(Tag::create_bool(self, text.get().is_empty()).into())
        })
    }
    fn text_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            let length = text.get().graphemes(true).count();
            Return(Int::create(self, length).into())
        })
    }
    fn text_starts_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, prefix: Text| {
            Return(Tag::create_bool(self, text.get().starts_with(prefix.get())).into())
        })
    }
    fn text_trim_end(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(Text::create(self, text.get().trim_end()).into())
        })
    }
    fn text_trim_start(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(Text::create(self, text.get().trim_start()).into())
        })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_debug_text(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            Return(Text::create(self, &format!("{:?}", **value)).into())
        })
    }

    fn try_(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |body: Function| { Try { body: *body } })
    }

    fn type_of(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            let type_name = match **value {
                Data::Int(_) => "Int",
                Data::Text(_) => "Text",
                Data::Tag(_) => "Tag",
                Data::List(_) => "List",
                Data::Struct(_) => "Struct",
                Data::HirId(_) => panic!(
                    "HIR ID shouldn't occurr in Candy programs except in VM-controlled places."
                ),
                Data::Function(_) => "Function",
                Data::Builtin(_) => "Builtin",
                Data::SendPort(_) => "SendPort",
                Data::ReceivePort(_) => "ReceivePort",
            };
            Return(Tag::create_from_str(self, type_name, None).into())
        })
    }
}

impl Function {
    fn should_take_no_arguments(&self) -> Result<(), String> {
        match self.argument_count() {
            0 => Ok(()),
            n => Err(format!("A builtin function expected a function without arguments, but got one that takes {n} arguments.")),
        }
    }
}

#[derive(Deref)]
struct UnpackedData<T> {
    object: InlineObject,

    #[deref]
    data: T,
}

#[derive(Deref)]
struct Any {
    data: Data,
}
impl TryInto<Any> for InlineObject {
    type Error = String;

    fn try_into(self) -> Result<Any, Self::Error> {
        Ok(Any { data: self.into() })
    }
}

use crate::{
    heap::{Data, Function, Heap, HirId, InlineObject, Int, List, Struct, Tag, Text, ToDebugText},
    instructions::InstructionResult,
    vm::{CallHandle, MachineState, Panic},
};
use candy_frontend::{
    builtin_functions::BuiltinFunction,
    format::{MaxLength, Precedence},
};
use derive_more::Deref;
use itertools::Itertools;
use num_bigint::BigInt;
use paste::paste;
use std::{
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{span, Level};

/// Our language server talks to clients using the LSP on stdin/stdout. When it
/// is running, we can't print log messages / etc. on stdout since it messes up
/// the LSP's messages.
pub static CAN_USE_STDOUT: AtomicBool = AtomicBool::new(true);

impl MachineState {
    pub(super) fn run_builtin_function(
        &mut self,
        heap: &mut Heap,
        builtin_function: BuiltinFunction,
        args: &[InlineObject],
        responsible: HirId,
    ) -> InstructionResult {
        let result = span!(Level::TRACE, "Running builtin").in_scope(|| match &builtin_function {
            BuiltinFunction::Equals => heap.equals(args),
            BuiltinFunction::FunctionRun => Heap::function_run(args, responsible),
            BuiltinFunction::GetArgumentCount => heap.get_argument_count(args),
            BuiltinFunction::IfElse => heap.if_else(args, responsible),
            BuiltinFunction::IntAdd => heap.int_add(args),
            BuiltinFunction::IntBitLength => heap.int_bit_length(args),
            BuiltinFunction::IntBitwiseAnd => heap.int_bitwise_and(args),
            BuiltinFunction::IntBitwiseOr => heap.int_bitwise_or(args),
            BuiltinFunction::IntBitwiseXor => heap.int_bitwise_xor(args),
            BuiltinFunction::IntCompareTo => heap.int_compare_to(args),
            BuiltinFunction::IntDivideTruncating => heap.int_divide_truncating(args),
            BuiltinFunction::IntModulo => heap.int_modulo(args),
            BuiltinFunction::IntMultiply => heap.int_multiply(args),
            BuiltinFunction::IntParse => heap.int_parse(args),
            BuiltinFunction::IntRemainder => heap.int_remainder(args),
            BuiltinFunction::IntShiftLeft => heap.int_shift_left(args),
            BuiltinFunction::IntShiftRight => heap.int_shift_right(args),
            BuiltinFunction::IntSubtract => heap.int_subtract(args),
            BuiltinFunction::ListFilled => heap.list_filled(args),
            BuiltinFunction::ListGet => heap.list_get(args),
            BuiltinFunction::ListInsert => heap.list_insert(args),
            BuiltinFunction::ListLength => heap.list_length(args),
            BuiltinFunction::ListRemoveAt => heap.list_remove_at(args),
            BuiltinFunction::ListReplace => heap.list_replace(args),
            BuiltinFunction::Print => heap.print(args),
            BuiltinFunction::StructGet => heap.struct_get(args),
            BuiltinFunction::StructGetKeys => heap.struct_get_keys(args),
            BuiltinFunction::StructHasKey => heap.struct_has_key(args),
            BuiltinFunction::TagGetValue => heap.tag_get_value(args),
            BuiltinFunction::TagHasValue => heap.tag_has_value(args),
            BuiltinFunction::TagWithoutValue => heap.tag_without_value(args),
            BuiltinFunction::TagWithValue => heap.tag_with_value(args),
            BuiltinFunction::TextCharacters => heap.text_characters(args),
            BuiltinFunction::TextConcatenate => heap.text_concatenate(args),
            BuiltinFunction::TextContains => heap.text_contains(args),
            BuiltinFunction::TextEndsWith => heap.text_ends_with(args),
            BuiltinFunction::TextFromUtf8 => heap.text_from_utf8(args),
            BuiltinFunction::TextGetRange => heap.text_get_range(args),
            BuiltinFunction::TextIsEmpty => heap.text_is_empty(args),
            BuiltinFunction::TextLength => heap.text_length(args),
            BuiltinFunction::TextStartsWith => heap.text_starts_with(args),
            BuiltinFunction::TextTrimEnd => heap.text_trim_end(args),
            BuiltinFunction::TextTrimStart => heap.text_trim_start(args),
            BuiltinFunction::ToDebugText => heap.to_debug_text(args),
            BuiltinFunction::TypeOf => heap.type_of(args),
        });

        match result {
            Ok(Return(value)) => {
                self.data_stack.push(value);
                InstructionResult::Done
            }
            Ok(DivergeControlFlow {
                function,
                responsible,
            }) => self.call_function(function, &[], responsible),
            Ok(CallHandle(call)) => InstructionResult::CallHandle(call),
            Err(reason) => InstructionResult::Panic(Panic {
                reason,
                responsible: responsible.get().clone(),
            }),
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
    CallHandle(CallHandle),
}

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
                panic!("A builtin function was called with the wrong number of arguments.");
            };
            let ( $( $arg, )+ ): ( $( UnpackedData<$type>, )+ ) = ( $(
                UnpackedData {
                    object: $arg,
                    data: $arg.try_into().unwrap(),
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
                panic!("A builtin function was called with the wrong number of arguments.");
            };
            let ( $( $arg, )+ ): ( $( UnpackedData<$type>, )+ ) = ( $(
                UnpackedData {
                    object: $arg,
                    data: $arg.try_into().unwrap(),
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

#[allow(clippy::enum_glob_use)]
use SuccessfulBehavior::*;

impl Heap {
    fn equals(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Any, b: Any| {
            Return(Tag::create_bool(self, **a == **b).into())
        })
    }

    fn function_run(args: &[InlineObject], responsible: HirId) -> BuiltinResult {
        unpack!(self, args, |function: Any| {
            match **function {
                Data::Builtin(_) => {
                    // TODO: Replace with `unreachable!()` once we have guards
                    // for argument counts on the Candy side – there are no
                    // builtins without arguments.
                    return Err("`✨.functionRun` called with builtin".to_string());
                }
                Data::Function(function) => DivergeControlFlow {
                    function,
                    responsible,
                },
                Data::Handle(handle) => {
                    if handle.argument_count() != 0 {
                        return Err(
                            "`✨.functionRun` expects a function or handle without arguments"
                                .to_string(),
                        );
                    }
                    CallHandle(CallHandle {
                        handle,
                        arguments: vec![],
                        responsible,
                    })
                }
                _ => return Err("`✨.functionRun` expects a function or handle".to_string()),
            }
        })
    }
    fn get_argument_count(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |function: Any| {
            let count = match **function {
                Data::Builtin(builtin) => builtin.get().num_parameters(),
                Data::Function(function) => function.argument_count(),
                Data::Handle(handle) => handle.argument_count(),
                _ => return Err("`✨.getArgumentCount` expects a function or handle".to_string()),
            };
            Return(Int::create(self, true, count).into())
        })
    }

    fn if_else(&mut self, args: &[InlineObject], responsible: HirId) -> BuiltinResult {
        unpack!(self, args, |condition: Tag,
                             then: Function,
                             else_: Function| {
            let (run, dont_run) = if condition.try_into_bool(self).unwrap() {
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
            Return(a.add(self, *b).into())
        })
    }
    fn int_bit_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int| { Return(a.bit_length(self).into()) })
    }
    fn int_bitwise_and(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_and(self, *b).into())
        })
    }
    fn int_bitwise_or(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_or(self, *b).into())
        })
    }
    fn int_bitwise_xor(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.bitwise_xor(self, *b).into())
        })
    }
    fn int_compare_to(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Int, b: Int| {
            Return(a.compare_to(self, *b).into())
        })
    }
    fn int_divide_truncating(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            Return(dividend.int_divide_truncating(self, *divisor).into())
        })
    }
    fn int_modulo(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            Return(dividend.modulo(self, *divisor).into())
        })
    }
    fn int_multiply(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |factor_a: Int, factor_b: Int| {
            Return(factor_a.multiply(self, *factor_b).into())
        })
    }
    fn int_parse(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |text: Text| {
            let result = BigInt::from_str(text.get())
                .map(|int| {
                    text.drop(self);
                    Int::create_from_bigint(self, true, int).into()
                })
                .map_err(|_| {
                    Tag::create_with_value(
                        self,
                        true,
                        self.default_symbols().not_an_integer,
                        text.object,
                    )
                    .into()
                });
            Return(Tag::create_result(self, true, result).into())
        })
    }
    fn int_remainder(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |dividend: Int, divisor: Int| {
            Return(dividend.remainder(self, *divisor).into())
        })
    }
    fn int_shift_left(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Int, amount: Int| {
            Return(value.shift_left(self, *amount).into())
        })
    }
    fn int_shift_right(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Int, amount: Int| {
            Return(value.shift_right(self, *amount).into())
        })
    }
    fn int_subtract(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |minuend: Int, subtrahend: Int| {
            Return(minuend.subtract(self, *subtrahend).into())
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

            Return(List::create(self, true, &vec![item_object; length_usize]).into())
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
            Return(Int::create(self, true, list.len()).into())
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

    fn print(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |message: Text| {
            if CAN_USE_STDOUT.load(Ordering::Relaxed) {
                println!("{}", message.get());
            } else {
                eprintln!("{}", message.get());
            }
            Return(Tag::create_nothing(self).into())
        })
    }

    fn struct_get(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct, key: Any| {
            let value = struct_.get(key.object).unwrap();
            value.dup(self);
            Return(value)
        })
    }
    fn struct_get_keys(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct| {
            Return(List::create(self, true, struct_.keys()).into())
        })
    }
    fn struct_has_key(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |struct_: Struct, key: Any| {
            Return(Tag::create_bool(self, struct_.contains(key.object)).into())
        })
    }

    fn tag_get_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            let value = tag.value().unwrap();
            value.dup(self);
            Return(value)
        })
    }
    fn tag_has_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            Return(Tag::create_bool(self, tag.value().is_some()).into())
        })
    }
    fn tag_without_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            Return(tag.without_value().into())
        })
    }
    fn tag_with_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag, value: Any| {
            Return(Tag::create_with_value(self, true, tag.symbol(), value.object).into())
        })
    }

    fn text_characters(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.characters(self).into())
        })
    }
    fn text_concatenate(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Text, b: Text| {
            Return(a.concatenate(self, *b).into())
        })
    }
    fn text_contains(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, pattern: Text| {
            Return(text.contains(self, *pattern).into())
        })
    }
    fn text_ends_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, suffix: Text| {
            Return(text.ends_with(self, *suffix).into())
        })
    }
    fn text_from_utf8(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |bytes: List| {
            // TODO: Remove `u8` checks once we have `needs` ensuring that the bytes are valid.
            let real_bytes: Vec<_> = bytes
                .items()
                .iter()
                .map(|&it| {
                    Int::try_from(it)
                        .ok()
                        .and_then(Int::try_get)
                        .ok_or_else(|| format!("Value is not a byte: {it}."))
                })
                .try_collect()?;
            let text = String::from_utf8(real_bytes)
                .map(|it| {
                    bytes.drop(self);
                    Text::create(self, true, &it).into()
                })
                .map_err(|_| {
                    Tag::create_with_value(
                        self,
                        true,
                        self.default_symbols().not_utf8,
                        bytes.object,
                    )
                    .into()
                });
            Return(Tag::create_result(self, true, text).into())
        })
    }
    fn text_get_range(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(
            self,
            args,
            |text: Text, start_inclusive: Int, end_exclusive: Int| {
                Return(
                    text.get_range(self, *start_inclusive..*end_exclusive)
                        .into(),
                )
            }
        )
    }
    fn text_is_empty(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.is_empty(self).into())
        })
    }
    fn text_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.length(self).into())
        })
    }
    fn text_starts_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, prefix: Text| {
            Return(text.starts_with(self, *prefix).into())
        })
    }
    fn text_trim_end(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.trim_end(self).into())
        })
    }
    fn text_trim_start(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.trim_start(self).into())
        })
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_debug_text(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            let formatted = value
                .object
                .to_debug_text(Precedence::Low, MaxLength::Unlimited);
            Return(Text::create(self, true, &formatted).into())
        })
    }

    fn type_of(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            let type_text = match **value {
                Data::Int(_) => self.default_symbols().int,
                Data::Text(_) => self.default_symbols().text,
                Data::Tag(_) => self.default_symbols().tag,
                Data::List(_) => self.default_symbols().list,
                Data::Struct(_) => self.default_symbols().struct_,
                Data::HirId(_) => panic!(
                    "HIR ID shouldn't occurr in Candy programs except in VM-controlled places."
                ),
                Data::Function(_) => self.default_symbols().function,
                Data::Builtin(_) => self.default_symbols().builtin,
                Data::Handle(_) => self.default_symbols().function,
            };
            Return(Tag::create(type_text).into())
        })
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

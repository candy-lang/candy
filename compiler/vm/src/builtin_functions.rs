use crate::{
    heap::{
        Data, DisplayWithSymbolTable, Function, Heap, HirId, InlineObject, Int, List, Struct,
        SymbolId, SymbolTable, Tag, Text, ToDebugText,
    },
    instructions::InstructionResult,
    vm::{MachineState, Panic},
};
use candy_frontend::{
    builtin_functions::BuiltinFunction,
    format::{MaxLength, Precedence},
};
use derive_more::Deref;
use itertools::Itertools;
use num_bigint::BigInt;
use paste::paste;
use std::str::FromStr;
use tracing::info;

impl MachineState {
    pub(super) fn run_builtin_function(
        &mut self,
        builtin_function: BuiltinFunction,
        args: &[InlineObject],
        symbol_table: &SymbolTable,
    ) -> InstructionResult {
        let responsible: HirId = (*args.last().unwrap()).try_into().unwrap();
        let args = &args[..args.len() - 1];

        let result = match &builtin_function {
            BuiltinFunction::Equals => self.heap.equals(args),
            BuiltinFunction::FunctionRun => {
                let [callee] = args else { unreachable!() };
                self.call(*callee, &[responsible.into()], symbol_table);
                return InstructionResult::Done;
            }
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
            BuiltinFunction::Print => self.heap.print(args, symbol_table),
            BuiltinFunction::StructGet => self.heap.struct_get(args),
            BuiltinFunction::StructGetKeys => self.heap.struct_get_keys(args),
            BuiltinFunction::StructHasKey => self.heap.struct_has_key(args),
            BuiltinFunction::TagGetValue => self.heap.tag_get_value(args),
            BuiltinFunction::TagHasValue => self.heap.tag_has_value(args),
            BuiltinFunction::TagWithoutValue => self.heap.tag_without_value(args),
            BuiltinFunction::TagWithValue => self.heap.tag_with_value(args),
            BuiltinFunction::TextCharacters => self.heap.text_characters(args),
            BuiltinFunction::TextConcatenate => self.heap.text_concatenate(args),
            BuiltinFunction::TextContains => self.heap.text_contains(args),
            BuiltinFunction::TextEndsWith => self.heap.text_ends_with(args),
            BuiltinFunction::TextFromUtf8 => self.heap.text_from_utf8(symbol_table, args),
            BuiltinFunction::TextGetRange => self.heap.text_get_range(args),
            BuiltinFunction::TextIsEmpty => self.heap.text_is_empty(args),
            BuiltinFunction::TextLength => self.heap.text_length(args),
            BuiltinFunction::TextStartsWith => self.heap.text_starts_with(args),
            BuiltinFunction::TextTrimEnd => self.heap.text_trim_end(args),
            BuiltinFunction::TextTrimStart => self.heap.text_trim_start(args),
            BuiltinFunction::ToDebugText => self.heap.to_debug_text(args, symbol_table),
            BuiltinFunction::TypeOf => self.heap.type_of(args),
        };

        match result {
            Ok(Return(value)) => {
                self.data_stack.push(value);
                InstructionResult::Done
            }
            Ok(DivergeControlFlow {
                function,
                responsible,
            }) => self.call_function(function, &[responsible.into()]),
            Err(reason) => InstructionResult::Panic(Panic {
                reason,
                responsible: responsible.get().to_owned(),
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

use SuccessfulBehavior::*;

impl Heap {
    fn equals(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |a: Any, b: Any| {
            Return(Tag::create_bool(**a == **b).into())
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
            let count_without_responsibility = count - 1;
            Return(Int::create(self, true, count_without_responsibility).into())
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
            Return(a.compare_to(*b).into())
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
        unpack_and_later_drop!(self, args, |text: Text| {
            let result = match BigInt::from_str(text.get()) {
                Ok(int) => Ok(Int::create_from_bigint(self, true, int).into()),
                Err(err) => Err(Text::create(self, true, &ToString::to_string(&err)).into()),
            };
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

    fn print(&mut self, args: &[InlineObject], symbol_table: &SymbolTable) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |message: Any| {
            info!(
                "{}",
                message
                    .object
                    .to_debug_text(Precedence::Low, MaxLength::Unlimited, symbol_table)
            );
            Return(Tag::create_nothing().into())
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
            Return(Tag::create_bool(struct_.contains(key.object)).into())
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
            Return(Tag::create_bool(tag.value().is_some()).into())
        })
    }
    fn tag_without_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |tag: Tag| {
            Return(tag.without_value().into())
        })
    }
    fn tag_with_value(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack!(self, args, |tag: Tag, value: Any| {
            let symbol = tag.symbol_id();
            tag.object.drop(self);
            Return(Tag::create_with_value(self, true, symbol, value.object).into())
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
            Return(text.contains(*pattern).into())
        })
    }
    fn text_ends_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, suffix: Text| {
            Return(text.ends_with(*suffix).into())
        })
    }
    fn text_from_utf8(
        &mut self,
        symbol_table: &SymbolTable,
        args: &[InlineObject],
    ) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |bytes: List| {
            // TODO: Remove `u8` checks once we have `needs` ensuring that the bytes are valid.
            let bytes: Vec<_> = bytes
                .items()
                .iter()
                .map(|&it| {
                    Int::try_from(it)
                        .ok()
                        .and_then(|it| it.try_get())
                        .ok_or_else(|| {
                            format!(
                                "Value is not a byte: {}.",
                                DisplayWithSymbolTable::to_string(&it, symbol_table),
                            )
                        })
                })
                .try_collect()?;
            Return(Text::create_from_utf8(self, true, &bytes).into())
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
        unpack_and_later_drop!(self, args, |text: Text| { Return(text.is_empty().into()) })
    }
    fn text_length(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text| {
            Return(text.length(self).into())
        })
    }
    fn text_starts_with(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |text: Text, prefix: Text| {
            Return(text.starts_with(*prefix).into())
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
    fn to_debug_text(
        &mut self,
        args: &[InlineObject],
        symbol_table: &SymbolTable,
    ) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            let formatted =
                value
                    .object
                    .to_debug_text(Precedence::Low, MaxLength::Unlimited, symbol_table);
            Return(Text::create(self, true, &formatted).into())
        })
    }

    fn type_of(&mut self, args: &[InlineObject]) -> BuiltinResult {
        unpack_and_later_drop!(self, args, |value: Any| {
            let type_symbol_id = match **value {
                Data::Int(_) => SymbolId::INT,
                Data::Text(_) => SymbolId::TEXT,
                Data::Tag(_) => SymbolId::TAG,
                Data::List(_) => SymbolId::LIST,
                Data::Struct(_) => SymbolId::STRUCT,
                Data::HirId(_) => panic!(
                    "HIR ID shouldn't occurr in Candy programs except in VM-controlled places."
                ),
                Data::Function(_) => SymbolId::FUNCTION,
                Data::Builtin(_) => SymbolId::BUILTIN,
                Data::Handle(_) => SymbolId::FUNCTION,
            };
            Return(Tag::create(type_symbol_id).into())
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

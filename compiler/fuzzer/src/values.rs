use std::{cell::RefCell, rc::Rc};

use candy_frontend::builtin_functions;
use candy_vm::heap::{Data, Heap, I64BitLength, InlineObject, Int, List, Struct, Tag, Text};
use itertools::Itertools;
use num_bigint::RandBigInt;
use rand::{
    prelude::ThreadRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use rustc_hash::FxHashMap;

use super::input::Input;

pub fn generate_input<'h>(
    heap: Rc<RefCell<Heap<'h>>>,
    num_args: usize,
    symbols: &[Text<'h>],
) -> Input<'h> {
    let mut arguments = vec![];
    for _ in 0..num_args {
        let address = generate_value_with_complexity(
            &mut heap.borrow_mut(),
            &mut rand::thread_rng(),
            100.0,
            symbols,
        );
        arguments.push(address);
    }
    Input { heap, arguments }
}

fn generate_value_with_complexity<'h>(
    heap: &mut Heap<'h>,
    rng: &mut ThreadRng,
    mut complexity: f32,
    symbols: &[Text<'h>],
) -> InlineObject<'h> {
    match rng.gen_range(1..=5) {
        1 => Int::create_from_bigint(heap, rng.gen_bigint(10)).into(),
        2 => Text::create(heap, "test").into(),
        // TODO: This should support tags with values
        3 => Tag::create(heap, *symbols.choose(rng).unwrap(), None).into(),
        4 => {
            complexity -= 1.0;
            let mut items = vec![];
            while complexity > 10.0 {
                let item = generate_value_with_complexity(heap, rng, 10.0, symbols);
                items.push(item);
                complexity -= 10.0;
            }
            List::create(heap, &items).into()
        }
        5 => {
            complexity -= 1.0;
            let mut fields = FxHashMap::default();
            while complexity > 20.0 {
                let key = generate_value_with_complexity(heap, rng, 10.0, symbols);
                let value = generate_value_with_complexity(heap, rng, 10.0, symbols);
                fields.insert(key, value);
                complexity -= 20.0;
            }
            Struct::create(heap, &fields).into()
        }
        6 => builtin_functions::VALUES[rng.gen_range(0..builtin_functions::VALUES.len())].into(),
        _ => unreachable!(),
    }
}

pub fn generate_mutated_input<'h>(
    rng: &mut ThreadRng,
    input: &mut Input<'h>,
    symbols: &[Text<'h>],
) {
    let mut heap = input.heap.borrow_mut();
    let num_args = input.arguments.len();
    let argument = input.arguments.get_mut(rng.gen_range(0..num_args)).unwrap();
    *argument = generate_mutated_value(rng, &mut heap, *argument, symbols);
}
fn generate_mutated_value<'h>(
    rng: &mut ThreadRng,
    heap: &mut Heap<'h>,
    object: InlineObject<'h>,
    symbols: &[Text<'h>],
) -> InlineObject<'h> {
    if rng.gen_bool(0.1) {
        return generate_value_with_complexity(heap, rng, 100.0, symbols);
    }

    match object.into() {
        Data::Int(int) => {
            Int::create_from_bigint(heap, int.get().as_ref() + rng.gen_range(-10..10)).into()
        }
        Data::Text(text) => mutate_string(rng, heap, text.get().to_string()).into(),
        // TODO: This should support tags with values
        Data::Tag(_) => {
            assert!(!symbols.is_empty());
            Tag::create(heap, *symbols.choose(rng).unwrap(), None).into()
        }
        Data::List(list) => {
            let len = list.len();
            if rng.gen_bool(0.9) && len > 0 {
                let index = rng.gen_range(0..len);
                let new_item = generate_mutated_value(rng, heap, list.get(index), symbols);
                list.replace(heap, index, new_item).into()
            } else if rng.gen_bool(0.5) && len > 0 {
                list.remove(heap, rng.gen_range(0..len)).into()
            } else {
                let new_item = generate_value_with_complexity(heap, rng, 100.0, symbols);
                list.insert(heap, rng.gen_range(0..=len), new_item).into()
            }
        }
        Data::Struct(struct_) => {
            let len = struct_.len();
            if rng.gen_bool(0.9) && len > 0 {
                let index = rng.gen_range(0..len);
                let key = struct_.keys()[index];
                let value = generate_mutated_value(rng, heap, struct_.values()[index], symbols);
                struct_.insert(heap, key, value).into()
            // TODO: Support removing value from a struct
            // } else if rng.gen_bool(0.5) && len > 0 {
            //     struct_
            //         .remove(rng.gen_range(0..len));
            } else {
                let key = generate_value_with_complexity(heap, rng, 10.0, symbols);
                let value = generate_value_with_complexity(heap, rng, 100.0, symbols);
                struct_.insert(heap, key, value).into()
            }
        }
        Data::Builtin(_) => (*builtin_functions::VALUES.choose(rng).unwrap()).into(),
        Data::HirId(_) | Data::Function(_) | Data::SendPort(_) | Data::ReceivePort(_) => {
            panic!("Couldn't have been created for fuzzing.")
        }
    }
}
fn mutate_string<'h>(rng: &mut ThreadRng, heap: &mut Heap<'h>, mut string: String) -> Text<'h> {
    if rng.gen_bool(0.5) && !string.is_empty() {
        let start = string.floor_char_boundary(rng.gen_range(0..=string.len()));
        let end = string.floor_char_boundary(start + rng.gen_range(0..=(string.len() - start)));
        string.replace_range(start..end, "")
    } else {
        let insertion_point = string.floor_char_boundary(rng.gen_range(0..=string.len()));
        let string_to_insert = (0..rng.gen_range(0..10))
            .map(|_| {
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
                    .chars()
                    .choose(rng)
                    .unwrap()
            })
            .join("");
        string.insert_str(insertion_point, &string_to_insert)
    }
    Text::create(heap, &string)
}

pub fn complexity_of_input(input: &Input) -> usize {
    input
        .arguments
        .iter()
        .map(|argument| complexity_of_value(*argument))
        .sum()
}
fn complexity_of_value(object: InlineObject) -> usize {
    match object.into() {
        Data::Int(int) => match int {
            Int::Inline(int) => int.get().bit_length() as usize,
            Int::Heap(int) => int.get().bits() as usize,
        },
        Data::Text(text) => text.len() + 1,
        // TODO: This should support tags with values
        Data::Tag(tag) => tag.symbol().get().len(),
        Data::List(list) => {
            list.items()
                .iter()
                .map(|item| complexity_of_value(*item))
                .sum::<usize>()
                + 1
        }
        Data::Struct(struct_) => {
            struct_
                .iter()
                .map(|(_, key, value)| complexity_of_value(key) + complexity_of_value(value))
                .sum::<usize>()
                + 1
        }
        Data::HirId(_)
        | Data::Function(_)
        | Data::Builtin(_)
        | Data::SendPort(_)
        | Data::ReceivePort(_) => 1,
    }
}

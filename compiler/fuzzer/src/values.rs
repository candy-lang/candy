use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use candy_frontend::builtin_functions;
use candy_vm::heap::{Data, Heap, List, Pointer};
use itertools::Itertools;
use num_bigint::RandBigInt;
use rand::{
    prelude::ThreadRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use rustc_hash::FxHashMap;

use super::input::Input;

pub fn generate_input(heap: Rc<RefCell<Heap>>, num_args: usize, symbols: &[Pointer]) -> Input {
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

fn generate_value_with_complexity(
    heap: &mut Heap,
    rng: &mut ThreadRng,
    mut complexity: f32,
    symbols: &[Pointer],
) -> Pointer {
    match rng.gen_range(1..=5) {
        1 => heap.create_int(rng.gen_bigint(10)),
        2 => heap.create_text("test".to_string()),
        3 => *symbols.choose(rng).unwrap(),
        4 => {
            complexity -= 1.0;
            let mut items = vec![];
            while complexity > 10.0 {
                let item = generate_value_with_complexity(heap, rng, 10.0, symbols);
                items.push(item);
                complexity -= 10.0;
            }
            heap.create_list(items)
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
            heap.create_struct(fields)
        }
        6 => heap.create_builtin(
            builtin_functions::VALUES[rng.gen_range(0..builtin_functions::VALUES.len())],
        ),
        _ => unreachable!(),
    }
}

pub fn generate_mutated_input(rng: &mut ThreadRng, input: &mut Input, symbols: &[Pointer]) {
    let mut heap = input.heap.borrow_mut();
    let num_args = input.arguments.len();
    let argument = input.arguments.get_mut(rng.gen_range(0..num_args)).unwrap();
    *argument = generate_mutated_value(rng, &mut heap, *argument, symbols);
}
fn generate_mutated_value(
    rng: &mut ThreadRng,
    heap: &mut RefMut<Heap>,
    address: Pointer,
    symbols: &[Pointer],
) -> Pointer {
    if rng.gen_bool(0.1) {
        return generate_value_with_complexity(heap, rng, 100.0, symbols);
    }

    let mut data = heap.get(address).data.clone();
    match &mut data {
        Data::Int(int) => {
            int.value += rng.gen_range(-10..10);
        }
        Data::Text(text) => {
            mutate_string(rng, &mut text.value);
        }
        Data::Symbol(symbol) => {
            if !symbols.is_empty() {
                symbol.value = symbols.choose(rng).unwrap().to_string();
            }
        }
        Data::List(List { items }) => {
            if rng.gen_bool(0.9) && !items.is_empty() {
                let index = rng.gen_range(0..items.len());
                items[index] = generate_mutated_value(rng, heap, items[index], symbols);
            } else if rng.gen_bool(0.5) && !items.is_empty() {
                items.remove(rng.gen_range(0..items.len()));
            } else {
                items.insert(
                    rng.gen_range(0..=items.len()),
                    generate_value_with_complexity(heap, rng, 100.0, symbols),
                );
            }
        }
        Data::Struct(struct_) => {
            if rng.gen_bool(0.9) && !struct_.fields.is_empty() {
                let index = rng.gen_range(0..struct_.fields.len());
                let (_, _, value) = struct_.fields[index];
                struct_.fields[index].2 = generate_mutated_value(rng, heap, value, symbols);
            } else if rng.gen_bool(0.5) && !struct_.fields.is_empty() {
                struct_
                    .fields
                    .remove(rng.gen_range(0..struct_.fields.len()));
            } else {
                let key = generate_value_with_complexity(heap, rng, 10.0, symbols);
                let value = generate_value_with_complexity(heap, rng, 100.0, symbols);
                struct_.insert(heap, key, value);
            }
        }
        Data::Builtin(builtin) => {
            builtin.function = *builtin_functions::VALUES.choose(rng).unwrap();
        }
        Data::HirId(_) | Data::Closure(_) | Data::SendPort(_) | Data::ReceivePort(_) => {
            panic!("Couldn't have been created for fuzzing.")
        }
    }

    heap.create(data)
}
fn mutate_string(rng: &mut ThreadRng, string: &mut String) {
    if rng.gen_bool(0.5) && !string.is_empty() {
        let start = string.floor_char_boundary(rng.gen_range(0..=string.len()));
        let end = string.floor_char_boundary(start + rng.gen_range(0..=(string.len() - start)));
        string.replace_range(start..end, "");
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
        string.insert_str(insertion_point, &string_to_insert);
    }
}

pub fn complexity_of_input(input: &Input) -> usize {
    let heap = input.heap.borrow();
    input
        .arguments
        .iter()
        .map(|argument| complexity_of_value(&heap, *argument))
        .sum()
}
fn complexity_of_value(heap: &Ref<Heap>, address: Pointer) -> usize {
    match &heap.get(address).data {
        Data::Int(int) => int.value.bits() as usize,
        Data::Text(text) => text.value.len() + 1,
        Data::Symbol(symbol) => symbol.value.len(),
        Data::List(list) => {
            list.items
                .iter()
                .map(|item| complexity_of_value(heap, *item))
                .sum::<usize>()
                + 1
        }
        Data::Struct(struct_) => {
            struct_
                .iter()
                .map(|(key, value)| {
                    complexity_of_value(heap, key) + complexity_of_value(heap, value)
                })
                .sum::<usize>()
                + 1
        }
        Data::HirId(_) => 1,
        Data::Closure(_) => 1,
        Data::Builtin(_) => 1,
        Data::SendPort(_) => 1,
        Data::ReceivePort(_) => 1,
    }
}

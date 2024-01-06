use super::input::Input;
use candy_frontend::builtin_functions;
use candy_vm::heap::{Data, Heap, I64BitLength, InlineObject, Int, List, Struct, Tag, Text};
use extension_trait::extension_trait;
use itertools::Itertools;
use num_bigint::RandBigInt;
use rand::{
    prelude::ThreadRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};
use rustc_hash::FxHashMap;
use std::collections::hash_map;

impl Input {
    pub fn generate(heap: &mut Heap, num_args: usize, symbols: &[Text]) -> Self {
        let arguments = (0..num_args)
            .map(|_| InlineObject::generate(heap, &mut rand::thread_rng(), 5.0, symbols))
            .collect();
        Self::new(arguments)
    }
    pub fn mutated(&self, heap: &mut Heap, rng: &mut ThreadRng, symbols: &[Text]) -> Self {
        let mut arguments = self.arguments().to_owned();

        let index_to_mutate = rng.gen_range(0..arguments.len());
        for (index, argument) in arguments.iter_mut().enumerate() {
            if index == index_to_mutate {
                *argument = argument.generate_mutated(heap, rng, symbols);
            } else {
                argument.dup(heap);
            }
        }
        Self::new(arguments)
    }
    pub fn complexity(&self) -> usize {
        self.arguments()
            .iter()
            .map(|argument| argument.complexity())
            .sum()
    }
}

#[extension_trait]
impl InlineObjectGeneration for InlineObject {
    fn generate(
        heap: &mut Heap,
        rng: &mut ThreadRng,
        mut complexity: f32,
        symbols: &[Text],
    ) -> InlineObject {
        match rng.gen_range(1..=5) {
            1 => Int::create_from_bigint(heap, true, rng.gen_bigint(10)).into(),
            2 => Text::create(heap, true, "test").into(),
            3 => {
                if rng.gen_bool(0.2) {
                    let value = Self::generate(heap, rng, complexity - 10.0, symbols);
                    Tag::create_with_value(heap, true, *symbols.choose(rng).unwrap(), value).into()
                } else {
                    let symbol = *symbols.choose(rng).unwrap();
                    symbol.dup();
                    Tag::create(symbol).into()
                }
            }
            4 => {
                complexity -= 1.0;
                let mut items = vec![];
                while complexity > 10.0 {
                    let item = Self::generate(heap, rng, 10.0, symbols);
                    items.push(item);
                    complexity -= 10.0;
                }
                List::create(heap, true, &items).into()
            }
            5 => {
                complexity -= 1.0;
                let mut fields = FxHashMap::default();
                while complexity > 20.0 {
                    // Generate a key that is not already in the struct
                    let entry = loop {
                        let key = Self::generate(heap, rng, 10.0, symbols);
                        match fields.entry(key) {
                            hash_map::Entry::Occupied(_) => key.drop(heap),
                            hash_map::Entry::Vacant(entry) => break entry,
                        }
                    };

                    let value = Self::generate(heap, rng, 10.0, symbols);
                    entry.insert(value);
                    complexity -= 20.0;
                }
                Struct::create(heap, true, &fields).into()
            }
            6 => {
                // No `dup()` necessary since these are inline.
                builtin_functions::VALUES[rng.gen_range(0..builtin_functions::VALUES.len())].into()
            }
            _ => unreachable!(),
        }
    }
    #[allow(clippy::too_many_lines)]
    fn generate_mutated(
        self,
        heap: &mut Heap,
        rng: &mut ThreadRng,
        symbols: &[Text],
    ) -> InlineObject {
        if rng.gen_bool(0.1) {
            return Self::generate(heap, rng, 100.0, symbols);
        }

        match self.into() {
            Data::Int(int) => {
                Int::create_from_bigint(heap, true, int.get().as_ref() + rng.gen_range(-10..10))
                    .into()
            }
            Data::Text(text) => {
                let mut string = text.get().to_string();
                mutate_string(rng, &mut string);
                Text::create(heap, true, &string).into()
            }
            Data::Tag(tag) => {
                if rng.gen_bool(0.5) {
                    // New symbol, keep value
                    let symbol = *symbols.choose(rng).unwrap();
                    symbol.dup();

                    if let Some(value) = tag.value() {
                        value.dup(heap);
                    }

                    Tag::create_with_value_option(heap, true, symbol, tag.value()).into()
                } else if let Some(value) = tag.value() {
                    tag.symbol().dup();
                    if rng.gen_bool(0.9) {
                        // Keep symbol, mutate value
                        let value = value.generate_mutated(heap, rng, symbols);
                        Tag::create_with_value(heap, true, tag.symbol(), value).into()
                    } else {
                        // Keep symbol, remove value
                        tag.without_value().into()
                    }
                } else {
                    // Keep symbol, add value
                    tag.symbol().dup();
                    let value = Self::generate(heap, rng, 100.0, symbols);
                    Tag::create_with_value(heap, true, tag.symbol(), value).into()
                }
            }
            Data::List(list) => {
                let len = list.len();
                if len > 0 && rng.gen_bool(0.9) {
                    // Replace item
                    let index_to_mutate = rng.gen_range(0..len);
                    let new_item = list
                        .get(index_to_mutate)
                        .generate_mutated(heap, rng, symbols);
                    for (index, item) in list.items().iter().enumerate() {
                        if index != index_to_mutate {
                            item.dup(heap);
                        }
                    }
                    list.replace(heap, index_to_mutate, new_item).into()
                } else if len > 0 && rng.gen_bool(0.5) {
                    // Remove item
                    let new_list = list.remove(heap, rng.gen_range(0..len));
                    for item in new_list.items() {
                        item.dup(heap);
                    }
                    new_list.into()
                } else {
                    // Add item
                    for item in list.items() {
                        item.dup(heap);
                    }
                    let new_item = Self::generate(heap, rng, 100.0, symbols);
                    list.insert(heap, rng.gen_range(0..=len), new_item).into()
                }
            }
            Data::Struct(struct_) => {
                let len = struct_.len();
                if rng.gen_bool(0.9) && len > 0 {
                    // Mutate value
                    let index_to_mutate = rng.gen_range(0..len);
                    for key in struct_.keys() {
                        key.dup(heap);
                    }
                    for (index, value) in struct_.values().iter().enumerate() {
                        if index != index_to_mutate {
                            value.dup(heap);
                        }
                    }
                    let value =
                        struct_.values()[index_to_mutate].generate_mutated(heap, rng, symbols);
                    struct_
                        .replace_at_index(heap, index_to_mutate, value)
                        .into()
                // TODO: Support removing value from a struct
                // } else if rng.gen_bool(0.5) && len > 0 {
                //     struct_
                //         .remove(rng.gen_range(0..len));
                } else {
                    // Add entry
                    for key in struct_.keys() {
                        key.dup(heap);
                    }
                    for value in struct_.values() {
                        value.dup(heap);
                    }

                    // Generate a key that is not already in the struct
                    let key = loop {
                        let key = Self::generate(heap, rng, 10.0, symbols);
                        if struct_.contains(key) {
                            key.drop(heap);
                        } else {
                            break key;
                        }
                    };
                    let value = Self::generate(heap, rng, 100.0, symbols);
                    struct_.insert(heap, key, value).into()
                }
            }
            Data::Builtin(_) => {
                // No `dup()` necessary since these are inline.
                (*builtin_functions::VALUES.choose(rng).unwrap()).into()
            }
            Data::HirId(_) | Data::Function(_) | Data::Handle(_) => {
                panic!("Couldn't have been created for fuzzing.")
            }
        }
    }

    fn complexity(self) -> usize {
        match self.into() {
            Data::Int(int) => match int {
                Int::Inline(int) => int.get().abs().bit_length() as usize,
                Int::Heap(int) => int.get().bits().try_into().unwrap_or(usize::MAX),
            },
            Data::Text(text) => text.byte_len() + 1,
            Data::Tag(tag) => {
                1 + tag
                    .value()
                    .map(InlineObjectGeneration::complexity)
                    .unwrap_or_default()
            }
            Data::List(list) => {
                list.items()
                    .iter()
                    .map(|item| item.complexity())
                    .sum::<usize>()
                    + 1
            }
            Data::Struct(struct_) => {
                struct_
                    .iter()
                    .map(|(_, key, value)| key.complexity() + value.complexity())
                    .sum::<usize>()
                    + 1
            }
            Data::HirId(_) | Data::Function(_) | Data::Builtin(_) | Data::Handle(_) => 1,
        }
    }
}

fn mutate_string(rng: &mut ThreadRng, string: &mut String) {
    if rng.gen_bool(0.5) && !string.is_empty() {
        let start = string.floor_char_boundary(rng.gen_range(0..string.len()));
        let end = string.ceil_char_boundary(rng.gen_range((start + 1)..=string.len()));
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

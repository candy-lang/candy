use std::collections::HashMap;

use crate::{
    builtin_functions,
    vm::{Heap, Pointer},
};
use num_bigint::RandBigInt;
use rand::{prelude::ThreadRng, Rng};

pub fn generate_n_values(heap: &mut Heap, n: usize) -> Vec<Pointer> {
    let mut values = vec![];
    for _ in 0..n {
        values.push(generate_value(heap));
    }
    values
}

fn generate_value(heap: &mut Heap) -> Pointer {
    generate_value_with_complexity(heap, &mut rand::thread_rng(), 100.0)
}
fn generate_value_with_complexity(
    heap: &mut Heap,
    rng: &mut ThreadRng,
    mut complexity: f32,
) -> Pointer {
    match rng.gen_range(1..=5) {
        1 => heap.create_int(rng.gen_bigint(10)),
        2 => heap.create_text("test".to_string()),
        3 => heap.create_symbol("Test".to_string()),
        4 => {
            complexity -= 1.0;
            let mut fields = HashMap::new();
            while complexity > 20.0 {
                let key = generate_value_with_complexity(heap, rng, 10.0);
                let value = generate_value_with_complexity(heap, rng, 10.0);
                fields.insert(key, value);
                complexity -= 20.0;
            }
            heap.create_struct(fields)
        }
        5 => heap.create_builtin(
            builtin_functions::VALUES[rng.gen_range(0..builtin_functions::VALUES.len())],
        ),
        _ => unreachable!(),
    }
}

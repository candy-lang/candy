use crate::{
    builtin_functions,
    vm::{Heap, Packet, Pointer},
};
use num_bigint::RandBigInt;
use rand::{prelude::ThreadRng, Rng};
use rustc_hash::FxHashMap;

pub fn generate_n_values(n: usize) -> Vec<Packet> {
    let mut values = vec![];
    for _ in 0..n {
        values.push(generate_value());
    }
    values
}

fn generate_value() -> Packet {
    let mut heap = Heap::default();
    let value = generate_value_with_complexity(&mut heap, &mut rand::thread_rng(), 100.0);
    Packet {
        heap,
        address: value,
    }
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
            let mut fields = FxHashMap::default();
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

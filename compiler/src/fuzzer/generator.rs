use crate::{builtin_functions, vm::value::Value};
use im::HashMap;
use num_bigint::RandBigInt;
use rand::{prelude::ThreadRng, Rng};

pub fn generate_n_values(n: usize) -> Vec<Value> {
    let mut values = vec![];
    for _ in 0..n {
        values.push(generate_value());
    }
    values
}

fn generate_value() -> Value {
    generate_value_with_complexity(&mut rand::thread_rng(), 100.0)
}
fn generate_value_with_complexity(rng: &mut ThreadRng, mut complexity: f32) -> Value {
    match rng.gen_range(1..=5) {
        1 => Value::Int(rng.gen_bigint(10)),
        2 => Value::Text("test".to_string()),
        3 => Value::Symbol("Test".to_string()),
        4 => {
            complexity -= 1.0;
            let mut fields = HashMap::new();
            while complexity > 20.0 {
                let key = generate_value_with_complexity(rng, 10.0);
                let value = generate_value_with_complexity(rng, 10.0);
                fields.insert(key, value);
                complexity -= 20.0;
            }
            Value::Struct(fields)
        }
        5 => Value::Builtin(
            builtin_functions::VALUES[rng.gen_range(0..builtin_functions::VALUES.len())],
        ),
        _ => unreachable!(),
    }
}

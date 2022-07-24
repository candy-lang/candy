mod closure_fuzzer;
mod generator;
mod input_fuzzer;
mod utils;

pub use self::closure_fuzzer::{Fuzzer, Status};
use crate::{
    database::Database,
    fuzzer::input_fuzzer::{fuzz_input, ClosurePanic},
    input::Input,
    vm::value::Value,
};
use itertools::Itertools;
use log;
use std::fs;

pub async fn fuzz(db: &Database, input: Input) {
    let panics = fuzz_input(db, input.clone()).await;
    for ClosurePanic {
        closure_id,
        arguments,
        message,
        tracer,
        ..
    } in panics
    {
        log::error!("The fuzzer discovered an input that crashes {closure_id}:");
        log::error!(
            "Calling `{closure_id} {}` doesn't work because {}.",
            arguments.iter().map(|it| format!("{}", it)).join(" "),
            match message {
                Value::Text(message) => message,
                other => format!("{}", other),
            },
        );
        log::error!("This was the stack trace:");
        tracer.dump_stack_trace(&db, input.clone());

        let trace = tracer.dump_call_tree();
        let mut trace_file = input.to_path().unwrap();
        trace_file.set_extension("candy.trace");
        fs::write(trace_file.clone(), trace).unwrap();
        log::info!(
            "Trace has been written to `{}`.",
            trace_file.as_path().display()
        );
    }
}

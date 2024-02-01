#![feature(absolute_path)]
#![allow(unused_attributes)]

use candy_frontend::module::PackagesPath;
use candy_vm::{
    byte_code::ByteCode,
    environment::Environment,
    heap::{Data, Handle, Heap, InlineObject, Int, List, Struct, Tag, Text},
    tracer::{stack_trace::StackTracer, Tracer},
    Vm, VmFinished, VmHandleCall,
};
use environment::BenchmarkingEnvironment;
use iai_callgrind::{black_box, library_benchmark, library_benchmark_group, main};
use itertools::Itertools;
use std::{
    borrow::Borrow,
    fs, iter,
    path::{self},
};
use tracing::Level;
use tracing_subscriber::{
    filter,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};
use utils::{compile, setup, setup_and_compile, Database};

mod environment;
mod utils;

#[library_benchmark]
#[bench::main(prepare("Examples/helloWorld.candy", &[]))]
fn compilation_hello_world(program: PreparedProgram) {
    run_program(program);
}
#[library_benchmark]
#[bench::main(prepare("Examples/fibonacci.candy", &["15"]))]
fn compilation_fibonacci(program: PreparedProgram) {
    run_program(program);
}

struct PreparedProgram {
    db: Database,
    byte_code: ByteCode,
    heap: Heap,
    environment: BenchmarkingEnvironment,
    environment_argument: Struct,
}
fn prepare(file_path: &str, arguments: &[&str]) -> PreparedProgram {
    init_logger();

    let source_code = fs::read_to_string(format!("../../packages/{file_path}")).unwrap();

    // let byte_code = setup_and_compile(&source_code);
    let mut db = setup();
    let byte_code = compile(&mut db, &source_code);

    let mut heap = Heap::default();
    let (environment_argument, environment) = BenchmarkingEnvironment::new(&mut heap, arguments);

    PreparedProgram {
        db,
        byte_code,
        heap,
        environment,
        environment_argument,
    }
}

fn run_program(mut program: PreparedProgram) {
    let vm = Vm::for_main_function(
        program.byte_code,
        &mut program.heap,
        program.environment_argument,
        StackTracer::default(),
    );
    let VmFinished { result, tracer, .. } =
        black_box(vm.run_forever_with_environment(&mut program.heap, &mut program.environment));
    result.unwrap_or_else(|it| {
        eprintln!("The program panicked: {}", it.reason);
        eprintln!("{} is responsible.", it.responsible);
        eprintln!(
            "This is the stack trace:\n{}",
            tracer.format(
                &program.db,
                &PackagesPath::try_from("../../packages").unwrap()
            ),
        );
        panic!("The program panicked: {}", it.reason)
    });
}

// benchmarks = compilation_hello_world, compilation_fibonacci
#[allow(unused_mut)]
library_benchmark_group!(
    name = compilation_group;
    benchmarks = compilation_hello_world, compilation_fibonacci
);
#[allow(unused_mut)]
main!(library_benchmark_groups = compilation_group);

fn init_logger() {
    let writer = BoxMakeWriter::new(std::io::stderr);
    let console_log = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(writer)
        .with_span_events(FmtSpan::ENTER)
        .with_filter(filter::filter_fn(|metadata| {
            metadata.level() <= &Level::WARN
        }));
    tracing_subscriber::registry().with(console_log).init();
}

#![feature(absolute_path)]
#![allow(unused_attributes)]

use candy_frontend::module::PackagesPath;
use candy_vm::{
    byte_code::ByteCode,
    heap::{Heap, Struct},
    tracer::stack_trace::StackTracer,
    Vm, VmFinished,
};
use environment::BenchmarkingEnvironment;
use iai_callgrind::{
    black_box, library_benchmark, library_benchmark_group, main, FlamegraphConfig,
    LibraryBenchmarkConfig,
};
use std::fs;
use tracing::Level;
use tracing_subscriber::{
    filter,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};
use utils::{compile, setup, Database};

mod environment;
mod utils;

#[library_benchmark]
#[bench::examples_fibonacci(compile_prepare("Examples/fibonacci.candy"))]
#[bench::examples_hello_world(compile_prepare("Examples/helloWorld.candy"))]
fn compile((mut db, source_code): (Database, String)) {
    utils::compile(&mut db, &source_code);
}
fn compile_prepare(file_path: &str) -> (Database, String) {
    init_logger();

    let db = setup();
    let source_code = fs::read_to_string(format!("../../packages/{file_path}")).unwrap();
    (db, source_code)
}

#[library_benchmark]
#[bench::examples_fibonacci(vm_runtime_prepare("Examples/fibonacci.candy", &[]))]
#[bench::examples_hello_world(vm_runtime_prepare("Examples/helloWorld.candy", &["15"]))]
fn vm_runtime(mut program: PreparedProgram) {
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

struct PreparedProgram {
    db: Database,
    byte_code: ByteCode,
    heap: Heap,
    environment: BenchmarkingEnvironment,
    environment_argument: Struct,
}
fn vm_runtime_prepare(file_path: &str, arguments: &[&str]) -> PreparedProgram {
    init_logger();

    let source_code = fs::read_to_string(format!("../../packages/{file_path}")).unwrap();

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

#[allow(unused_mut)]
library_benchmark_group!(
    name = main;
    benchmarks = compile, vm_runtime
);
#[allow(unused_mut)]
main!(
    config = LibraryBenchmarkConfig::default().flamegraph(FlamegraphConfig::default());
    library_benchmark_groups = main
);

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

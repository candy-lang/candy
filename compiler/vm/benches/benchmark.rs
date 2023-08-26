use criterion::{
    criterion_group, criterion_main, measurement::Measurement, BatchSize, Bencher, BenchmarkId,
    Criterion,
};
use criterion_cycles_per_byte::CyclesPerByte;
use tracing::Level;
use tracing_subscriber::{
    filter,
    fmt::{format::FmtSpan, writer::BoxMakeWriter},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};
use utils::{compile, run, setup, setup_and_compile};

mod utils;

fn benchmark_compiler<M: Measurement>(c: &mut Criterion<M>, prefix: &str) {
    let mut group = c.benchmark_group(format!("{prefix}: Compiler"));

    group.sample_size(100);
    group.bench_function("hello_world", |b| {
        b.compile(r#"main _ := "Hello, world!""#);
    });

    group.sample_size(20);
    let fibonacci_code = create_fibonacci_code(15);
    group.bench_function("fibonacci", |b| b.compile(&fibonacci_code));

    group.finish();
}
fn benchmark_vm_runtime<M: Measurement>(c: &mut Criterion<M>, prefix: &str) {
    let mut group = c.benchmark_group(format!("{prefix}: VM Runtime"));

    // This is a macro so that we can accept a string or `BenchmarkId`.
    macro_rules! benchmark {
        ($id:expr, $source_code:expr, $sample_size:expr $(,)?) => {
            group.sample_size($sample_size);
            group.bench_function($id, |b| b.run_vm($source_code));
        };
        ($id:expr, $parameter:expr, $source_code_factory:expr, $sample_size:expr $(,)?) => {
            benchmark!(
                BenchmarkId::new($id, $parameter),
                &$source_code_factory($parameter),
                $sample_size,
            );
        };
    }

    benchmark!("hello_world", r#"main _ := "Hello, world!""#, 100);
    benchmark!("fibonacci", 15, create_fibonacci_code, 20);
    benchmark!("PLB/binarytrees", 6, create_binary_trees_code, 10);

    group.finish();
}

fn create_fibonacci_code(n: usize) -> String {
    format!(
        r#"[ifElse, int] = use "Core"

fibRec = {{ fibRec n ->
  ifElse (n | int.isLessThan 2) {{ n }} {{
    fibRec fibRec (n | int.subtract 1)
    | int.add (fibRec fibRec (n | int.subtract 2))
  }}
}}
fib n =
  needs (int.is n)
  fibRec fibRec n

main _ := fib {n}"#,
    )
}
/// https://programming-language-benchmarks.vercel.app/problem/binarytrees
fn create_binary_trees_code(n: usize) -> String {
    format!(
        r#"
[equals, if, ifElse, int, iterable, recursive, result, struct, text] = use "Core"

createTree n :=
  needs (int.is n)
  needs (int.isNonNegative n)

  recursive n {{ recurse n ->
    ifElse (n | equals 0) {{ [] }} {{
      nextSize = n | int.subtract 1
      [Left: recurse nextSize, Right: recurse nextSize]
    }}
  }}
checkTree tree :=
  needs (struct.is tree)

  recursive tree {{ recurse tree ->
    left = tree | struct.get Left | result.mapOr {{ it -> recurse it }} 0
    right = tree | struct.get Right | result.mapOr {{ it -> recurse it }} 0
    1 | int.add left | int.add right
  }}

main _ :=
  n = {n}
  minDepth = 4

  maxDepth = n | int.coerceAtLeast (minDepth | int.add 2)
  _ =
    depth = maxDepth | int.add 1
    tree = createTree depth

  longLivedTree = createTree maxDepth

  recursive minDepth {{ recurse depth ->
    if (depth | int.isLessThanOrEqualTo maxDepth) {{
      iterations = 1 | int.shiftLeft (maxDepth | int.subtract depth | int.add minDepth)
      check = iterable.generate iterations {{ _ -> createTree depth | checkTree }} | iterable.sum
      recurse (depth | int.add 2)
    }}
  }}
"#,
    )
}

trait BencherExtension {
    fn compile(&mut self, source_code: &str);
    fn run_vm(&mut self, source_code: &str);
}
impl<'a, M: Measurement> BencherExtension for Bencher<'a, M> {
    fn compile(&mut self, source_code: &str) {
        self.iter_batched(
            setup,
            |mut db| compile(&mut db, source_code),
            BatchSize::SmallInput,
        )
    }
    fn run_vm(&mut self, source_code: &str) {
        self.iter_batched(
            || setup_and_compile(source_code),
            run,
            BatchSize::SmallInput,
        )
    }
}

fn run_benchmarks<M: Measurement>(c: &mut Criterion<M>, prefix: &str) {
    init_logger();
    benchmark_compiler(c, prefix);
    benchmark_vm_runtime(c, prefix);
}

fn run_cycle_benchmarks(c: &mut Criterion<CyclesPerByte>) {
    init_logger();
    run_benchmarks(c, "Cycles");
}
criterion_group!(
    name = cycle_benchmarks;
    config = Criterion::default().with_measurement(CyclesPerByte);
    targets = run_cycle_benchmarks,
);

fn run_time_benchmarks(c: &mut Criterion) {
    run_benchmarks(c, "Time");
}
criterion_group!(time_benchmarks, run_time_benchmarks);

// criterion_main!(cycle_benchmarks, time_benchmarks);
criterion_main!(time_benchmarks);

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

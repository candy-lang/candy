use criterion::{
    criterion_group, criterion_main, measurement::Measurement, BatchSize, Bencher, BenchmarkId,
    Criterion,
};
use utils::{compile, run, setup, setup_and_compile};

mod utils;

// `Core` is available via `use "..Core"`.

fn benchmark_compiler(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compiler");

    group.sample_size(100);
    group.bench_function("hello_world", |b| {
        b.compile(r#"main _ := "Hello, world!""#);
    });

    group.sample_size(20);
    let fibonacci_code = create_fibonacci_code(15);
    group.bench_function("fibonacci", |b| b.compile(&fibonacci_code));

    group.finish();
}
fn benchmark_vm_runtime(c: &mut Criterion) {
    let mut group = c.benchmark_group("VM Runtime");

    group.sample_size(100);
    group.bench_function("hello_world", |b| {
        b.run_vm(r#"main _ := "Hello, world!""#);
    });

    group.sample_size(20);
    let n = 15;
    let fibonacci_code = create_fibonacci_code(n);
    group.bench_function(BenchmarkId::new("fibonacci", n), |b| {
        b.run_vm(&fibonacci_code)
    });

    group.finish();
}

fn create_fibonacci_code(n: usize) -> String {
    format!(
        r#"[ifElse, int] = use "..Core"

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

trait BencherExtension {
    fn compile(&mut self, source_code: &str);
    fn run_vm(&mut self, source_code: &str);
}
impl<'a, M: Measurement> BencherExtension for Bencher<'a, M> {
    fn compile(&mut self, source_code: &str) {
        self.iter_batched(
            || setup(),
            |mut db| compile(&mut db, source_code),
            BatchSize::SmallInput,
        )
    }
    fn run_vm(&mut self, source_code: &str) {
        self.iter_batched(
            || setup_and_compile(source_code),
            |(db, lir)| run(&db, lir),
            BatchSize::SmallInput,
        )
    }
}

criterion_group!(benches, benchmark_compiler, benchmark_vm_runtime);
criterion_main!(benches);

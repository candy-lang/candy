use criterion::{
    criterion_group, criterion_main, measurement::Measurement, BatchSize, Bencher, BenchmarkId,
    Criterion,
};
use utils::{run, setup_and_compile};

mod utils;

// `Core` is available via `use "..Core"`.

fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("basics");

    group.sample_size(20);
    group.bench_function("hello_world", |b| {
        b.run_candy(r#"main _ := "Hello, world!""#);
    });

    group.sample_size(10);
    let n = 15;
    let fibonacci_code = format!(
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

main _ := fib {n}"#
    );
    group.bench_function(BenchmarkId::new("fibonacci", n), |b| {
        b.run_candy(&fibonacci_code)
    });

    group.finish();
}

trait BencherExtension {
    fn run_candy(&mut self, source_code: &str);
}
impl<'a, M: Measurement> BencherExtension for Bencher<'a, M> {
    fn run_candy(&mut self, source_code: &str) {
        self.iter_batched(
            || setup_and_compile(source_code),
            |(db, lir)| run(&db, lir),
            BatchSize::SmallInput,
        )
    }
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

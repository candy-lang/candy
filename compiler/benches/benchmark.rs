use candy::database::Database;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use utils::{compile_and_run, setup};

mod utils;

fn hello_world(db: &mut Database, message: &str) {
    compile_and_run(db, &format!("main _ := \"{message}\""));
}

fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("basics");

    group.sample_size(20);
    group.bench_function("hello_world", |b| {
        b.iter_batched_ref(
            setup,
            |db| hello_world(db, black_box("Hello, world!")),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);

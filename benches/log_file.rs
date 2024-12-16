use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fastlog::Config;

fn bench_log_file(c: &mut Criterion) {
    let _ = std::fs::remove_file("target/test_bench.log");
    fastlog::init(
        Config::new()
            .file("target/test_bench.log")
            .chan_len(Some(1000000)),
    )
    .unwrap();

    c.bench_function("bench_log_file", |b| {
        b.iter(|| {
            black_box(log::info!("Commencing yak shaving"));
        });

        log::logger().flush();
    });
}

criterion_group!(benches,
    bench_log_file,
);
criterion_main!(benches);
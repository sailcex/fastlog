use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fastlog::appender::{FastLogRecord, LogAppender};
use fastlog::Config;

fn bench_log(c: &mut Criterion) {
    struct BenchRecvLog {}
    impl LogAppender for BenchRecvLog {
        fn do_logs(&mut self, _records: &[FastLogRecord]) {
            //nothing
        }
    }
    fastlog::init(
        Config::new()
            .custom(BenchRecvLog {})
            .chan_len(Some(1000000)),
    )
    .unwrap();

    c.bench_function("bench_log", |b| {
        b.iter(|| {
            black_box(log::info!("Commencing yak shaving"));
        });
    });
}

criterion_group!(benches,
    bench_log,
);
criterion_main!(benches);
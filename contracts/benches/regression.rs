use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Instant;
use your_crate_name::*;

fn regression_test_add() {
    let start = Instant::now();
    for _ in 0..1_000_000 {
        add(10, 20);
    }
    let duration = start.elapsed();

    assert!(
        duration.as_millis() < 50,
        "Performance regression detected in add()"
    );
}

fn bench_regression(c: &mut Criterion) {
    c.bench_function("regression_test_add", |b| b.iter(|| regression_test_add()));
}

criterion_group!(regression_group, bench_regression);
criterion_main!(regression_group);

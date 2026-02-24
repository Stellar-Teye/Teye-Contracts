use criterion::{black_box, criterion_group, criterion_main, Criterion};
use your_crate_name::*;

fn bench_add(c: &mut Criterion) {
    c.bench_function("add", |b| {
        b.iter(|| add(black_box(10), black_box(20)))
    });
}

fn bench_expensive_computation(c: &mut Criterion) {
    c.bench_function("expensive_computation", |b| {
        b.iter(|| expensive_computation(black_box(10_000)))
    });
}

fn bench_storage_simulation(c: &mut Criterion) {
    c.bench_function("storage_simulation", |b| {
        b.iter(|| {
            let mut data = vec![];
            storage_simulation(&mut data, black_box(42))
        })
    });
}

criterion_group!(
    benches,
    bench_add,
    bench_expensive_computation,
    bench_storage_simulation
);
criterion_main!(benches);

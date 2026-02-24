use criterion::{criterion_group, criterion_main, Criterion};
use your_crate_name::*;

fn bench_gas_simulation(c: &mut Criterion) {
    c.bench_function("gas_sim_expensive_computation", |b| {
        b.iter(|| {
            // Simulate gas-heavy function
            expensive_computation(50_000)
        })
    });
}

criterion_group!(gas_benches, bench_gas_simulation);
criterion_main!(gas_benches);

//! SPEC §2.3 指标 1：宿主冷启动开销（engine + component 加载 + instantiate + 1 次 call）。
//! 基准 < 100 ms。criterion 测法（SPEC 表格指定）。

use criterion::{criterion_group, criterion_main, Criterion};
use partisync_wasm_spike::host;
use std::hint::black_box;

fn bench_cold_start(c: &mut Criterion) {
    let payload = host::ten_kb_json();
    c.bench_function("wasm_cold_start_load_and_call", |b| {
        b.iter(|| {
            let out = host::cold_start_once(black_box(&payload)).expect("cold start");
            black_box(out);
        })
    });
}

criterion_group!(benches, bench_cold_start);
criterion_main!(benches);

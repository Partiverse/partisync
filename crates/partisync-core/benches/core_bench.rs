//! 核心类型基准（SPEC M0-WP00，D2 基线机制）。
//!
//! 用法：`scripts/bench.sh save m0` 建基线；`scripts/bench.sh check m0` 对照回归。

use std::str::FromStr;

use criterion::{criterion_group, criterion_main, Criterion};
use partisync_core::Ulid;

fn bench_ulid(c: &mut Criterion) {
    let sample = Ulid::from_parts(0x0123_4567_89AB, 0x0FED_CBA9_8765_4321_0011_2233);

    c.bench_function("ulid/encode_display_26", |b| {
        b.iter(|| std::hint::black_box(sample.to_string()))
    });

    let encoded = sample.to_string();
    c.bench_function("ulid/parse_from_str", |b| {
        b.iter(|| std::hint::black_box(Ulid::from_str(&encoded).unwrap()))
    });

    c.bench_function("ulid/field_extraction", |b| {
        b.iter(|| {
            let u = std::hint::black_box(&sample);
            std::hint::black_box((u.timestamp_ms(), u.random_part()))
        })
    });
}

criterion_group!(benches, bench_ulid);
criterion_main!(benches);

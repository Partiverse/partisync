//! SPEC §2.3 指标 2：单次 tool call RTT（10 KB JSON 往返，已实例化）。
//! 基准 < 5 ms。criterion 测法（SPEC 表格指定）。

use criterion::{criterion_group, criterion_main, Criterion};
use partisync_wasm_spike::host::{self, DemoTool};
use std::hint::black_box;

fn bench_call_rtt(c: &mut Criterion) {
    let engine = host::engine().expect("engine");
    let component = host::component(&engine, "demo_tool.wasm").expect("component");
    let linker = host::deny_linker(&engine);
    let mut tool = DemoTool::instantiate(&engine, &component, &linker).expect("instantiate");
    let payload = host::ten_kb_json();

    c.bench_function("wasm_call_rtt_10kb_json", |b| {
        b.iter(|| {
            let out = tool.call(black_box(&payload)).expect("call");
            black_box(out);
        })
    });
}

criterion_group!(benches, bench_call_rtt);
criterion_main!(benches);

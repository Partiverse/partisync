//! 六项基准汇总报告（T01 报告数字来源）：冷启动 min/P50/P95、call RTT
//! min/P50/P95、拒权探针结论。RSS 与 MSRV/deny 由外部命令测（README）。
use partisync_wasm_spike::host::{self, DemoTool};
use std::time::Instant;
use wasmtime::Store;

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (((v.len() - 1) as f64) * p).round() as usize;
    v[idx]
}

fn main() -> Result<(), wasmtime::Error> {
    let payload = host::ten_kb_json();
    println!("payload bytes: {}", payload.len());

    // 指标 1：冷启动（20 次）
    let mut cold: Vec<f64> = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        host::cold_start_once(&payload)?;
        cold.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "cold_start_ms  min={:.1} p50={:.1} p95={:.1} max={:.1}",
        cold.iter().cloned().fold(f64::INFINITY, f64::min),
        pct(&mut cold, 0.50),
        pct(&mut cold, 0.95),
        cold.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );

    // 指标 2：call RTT（1000 次，已实例化）
    let engine = host::engine()?;
    let component = host::component(&engine, "demo_tool.wasm")?;
    let linker = host::deny_linker(&engine);
    let mut tool = DemoTool::instantiate(&engine, &component, &linker)?;
    let mut rtt: Vec<f64> = Vec::new();
    for _ in 0..1000 {
        let t = Instant::now();
        let out = tool.call(&payload)?;
        rtt.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&out);
    }
    println!(
        "call_rtt_ms    min={:.3} p50={:.3} p95={:.3} max={:.3}",
        rtt.iter().cloned().fold(f64::INFINITY, f64::min),
        pct(&mut rtt, 0.50),
        pct(&mut rtt, 0.95),
        rtt.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );

    // 指标 4：拒权探针（与 tests/deny_probes.rs 同逻辑的快速结论行）
    let probe_component = host::component(&engine, "probe_deny.wasm")?;
    let probe_linker = host::deny_linker(&engine);
    let mut probe_store = Store::new(&engine, ());
    match probe_linker.instantiate(&mut probe_store, &probe_component) {
        Err(e) => {
            let text = format!("{e}");
            let leak = ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"]
                .iter()
                .any(|s| text.contains(s));
            let head: String = text.chars().take(140).collect();
            println!("deny_probe     REJECTED (leak={leak}) err_head={head}");
        }
        Ok(_) => {
            eprintln!("拒权探针未被拒绝：默认拒权不成立");
            std::process::exit(1);
        }
    }
    Ok(())
}

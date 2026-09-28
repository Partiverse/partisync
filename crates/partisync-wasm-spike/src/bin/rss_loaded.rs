//! 装载态内存：engine + demo component + instantiate + 1 次 call。
//! 供 `/usr/bin/time -l` 测 peak RSS（SPEC §2.3 指标 3）。
fn main() {
    let payload = partisync_wasm_spike::host::ten_kb_json();
    match partisync_wasm_spike::host::cold_start_once(&payload) {
        Ok(out) => println!("loaded, call returned {} bytes", out.len()),
        Err(e) => {
            eprintln!("load failed: {e:#}");
            std::process::exit(1);
        }
    }
}

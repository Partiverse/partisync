//! WP01 宏基准（SPEC M3-WP01 验收「盘上预算基准」「基准报告（T06）」）。
//!
//! 非 criterion 微基准——10⁷ 量级导入是单遍宏测量（M2 KPI 同款口径：
//! 参数驱动 + Instant 计时 + 报告人工登记）。用法（release 下运行）：
//!
//! ```text
//! cargo bench -p partisync-hub --bench wp01 -- import 1000000   # 顺序导入 N 条
//! cargo bench -p partisync-hub --bench wp01 -- list 2000        # LIST 256 页延迟分布
//! cargo bench -p partisync-hub --bench wp01 -- point 2000       # 点查延迟分布
//! cargo bench -p partisync-hub --bench wp01 -- budget           # 盘上字节/条目
//! cargo bench -p partisync-hub --bench wp01 -- lifecycle 3      # Hub open+drop 计时
//! ```
//!
//! 库根默认 `{CARGO_TARGET_TMPDIR}/wp01-bench`（import 写入 bench-meta.txt
//! 供 budget/list 复用；cargo clean 即回收）。ID 为 seq 的纯函数
//! （ULID 形状：6B 毫秒时间戳 + splitmix 字节 + seq），list/point 可离线重放。

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use partisync_hub::{encode_entry_row, EntryRow, Hub, KIND_DIR, KIND_FILE};

/// 每目录子项数（10⁷ → 10⁴ 目录 × 1000 子项，LIST 分页语义的代表性形态）。
const CHILDREN_PER_DIR: u64 = 1000;
/// LIST/point 延迟分布的页大小（SPEC §4：单目录 p99 由分页保证）。
const PAGE: u32 = 256;

fn bench_root() -> PathBuf {
    let base = option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("wp01-bench")
}

/// splitmix64（确定性、无依赖；仅基准数据生成用，非路由哈希）。
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 第 `seq` 个条目的平面 ID：ULID 形状（6B 时间戳 + 6B splitmix + 4B seq）。
fn id_of(seq: u64) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[..6].copy_from_slice(&1_726_800_000u64.to_be_bytes()[2..]);
    let mut sm = seq ^ 0xA5A5_5A5A_1234_5678;
    id[6..12].copy_from_slice(&splitmix64(&mut sm).to_be_bytes()[2..]);
    id[12..].copy_from_slice(&seq.to_be_bytes()[4..]);
    id
}

fn dir_count(n: u64) -> u64 {
    (n / CHILDREN_PER_DIR).max(1)
}

fn dir_row(seq: u64) -> EntryRow {
    EntryRow {
        entry_id: id_of(seq),
        parent_id: None,
        kind: KIND_DIR,
        name: format!("dir-{seq:06}"),
        content_id: None,
        size: 0,
        mtime_ns: 1_726_800_000_000_000_000,
        flags: 0,
    }
}

fn file_row(dir_seq: u64, nth: u64) -> EntryRow {
    let seq = dir_seq * CHILDREN_PER_DIR + nth;
    let mut sm = seq ^ 0xDEAD_BEEF_1234_5678;
    let r1 = splitmix64(&mut sm);
    let r2 = splitmix64(&mut sm);
    let mut content = [0u8; 32];
    content[..8].copy_from_slice(&r1.to_be_bytes());
    content[8..16].copy_from_slice(&r2.to_be_bytes());
    EntryRow {
        entry_id: id_of(1 << 32 | seq), // 文件与目录 id 空间分离，point 可重放
        parent_id: Some(id_of(dir_seq)),
        kind: KIND_FILE,
        name: format!("asset-{seq:08}-{r1:016x}"),
        content_id: Some(content),
        size: r2 % 1_000_000,
        mtime_ns: 1_726_800_000_000_000_000 + (r2 % 86_400_000_000_000) as i64,
        flags: 0,
    }
}

fn file_id(dir_seq: u64, nth: u64) -> [u8; 16] {
    id_of(1 << 32 | (dir_seq * CHILDREN_PER_DIR + nth))
}

fn meta_path(root: &Path) -> PathBuf {
    root.join("bench-meta.txt")
}

fn read_meta(root: &Path) -> (u64, u64, u64) {
    let text = fs::read_to_string(meta_path(root)).expect("bench-meta.txt（先跑 import）");
    let mut n = 0u64;
    let mut logical = 0u64;
    let mut dirs = 0u64;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("n=") {
            n = v.parse().expect("n");
        }
        if let Some(v) = line.strip_prefix("logical=") {
            logical = v.parse().expect("logical");
        }
        if let Some(v) = line.strip_prefix("dirs=") {
            dirs = v.parse().expect("dirs");
        }
    }
    (n, logical, dirs)
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

fn percentile(mut xs: Vec<u128>) -> (u128, u128, u128, u128) {
    xs.sort_unstable();
    let p = |q: f64| -> u128 { xs[(q * (xs.len() - 1) as f64).round() as usize] };
    (p(0.50), p(0.95), p(0.99), xs[xs.len() - 1])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or_else(|| {
        eprintln!("用法: wp01-bench <import N | list Q | point Q | budget | lifecycle [k]>");
        std::process::exit(2);
    });
    let root = bench_root();
    match cmd {
        "import" => {
            let n: u64 = args.get(2).expect("N").parse().expect("N 数值");
            import(&root, n);
        }
        "list" => {
            let q: u64 = args.get(2).map(|s| s.parse().expect("Q")).unwrap_or(2000);
            list(&root, q);
        }
        "point" => {
            let q: u64 = args.get(2).map(|s| s.parse().expect("Q")).unwrap_or(2000);
            point(&root, q);
        }
        "budget" => budget(&root),
        "lifecycle" => {
            let k: u64 = args.get(2).map(|s| s.parse().expect("k")).unwrap_or(3);
            lifecycle(&root, k);
        }
        other => {
            eprintln!("未知子命令: {other}");
            std::process::exit(2);
        }
    }
}

fn import(root: &Path, n: u64) {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root).expect("mkdir");
    let hub = Hub::open(root).expect("open");
    let dirs = dir_count(n);

    let mut logical: u64 = 0;
    let started = Instant::now();
    for dir_seq in 0..dirs {
        let d = dir_row(dir_seq);
        logical += 16 + encode_entry_row(&d).unwrap().len() as u64;
        hub.put_entry(&d).expect("put dir");
        for nth in 0..CHILDREN_PER_DIR {
            let f = file_row(dir_seq, nth);
            // 逻辑字节/条目 = entry 键+值 + children 键+值（SPEC §1 预算口径）
            logical += (16 + encode_entry_row(&f).unwrap().len()) as u64;
            logical += (16 + f.name.len() + 18) as u64;
            hub.put_entry(&f).expect("put file");
        }
        let done = (dir_seq + 1) * CHILDREN_PER_DIR;
        if done.is_multiple_of(100_000) {
            // 100 行进度曲线：分裂停顿在曲线上呈平台期
            let secs = started.elapsed().as_secs_f64();
            eprintln!(
                "import {done}/{n} ({:>4.1}%)  rate {:.0}/s",
                100.0 * done as f64 / n as f64,
                done as f64 / secs,
            );
        }
    }
    hub.persist().expect("persist");
    let secs = started.elapsed().as_secs_f64();
    let entries = dirs + dirs * CHILDREN_PER_DIR;
    println!("== import ==");
    println!("entries: {entries}");
    println!("dirs: {dirs}  children/dir: {CHILDREN_PER_DIR}");
    println!("wall_s: {secs:.2}");
    println!("throughput_eps: {:.0}", entries as f64 / secs);
    println!(
        "logical_bytes_per_entry: {:.1}",
        logical as f64 / entries as f64
    );
    drop(hub);
    let mut meta = fs::File::create(meta_path(root)).expect("meta");
    writeln!(meta, "n={entries}").unwrap();
    writeln!(meta, "logical={logical}").unwrap();
    writeln!(meta, "dirs={dirs}").unwrap();
}

fn list(root: &Path, q: u64) {
    let (_, _, dirs) = read_meta(root);
    let hub = Hub::open(root).expect("open");
    let mut sm = q ^ 0x1BAD_B002;
    let mut samples: Vec<u128> = Vec::with_capacity(q as usize);
    let mut items_total = 0u64;
    for _ in 0..q {
        let dir_seq = splitmix64(&mut sm) % dirs;
        let dir_id = id_of(dir_seq);
        let t = Instant::now();
        let page = hub.list_children(&dir_id, None, PAGE).expect("list");
        samples.push(t.elapsed().as_micros());
        items_total += page.items.len() as u64;
    }
    let (p50, p95, p99, max) = percentile(samples);
    println!("== list (page={PAGE}, q={q}) ==");
    println!("p50_us: {p50}");
    println!("p95_us: {p95}");
    println!("p99_us: {p99}");
    println!("max_us: {max}");
    println!("avg_items_per_page: {:.1}", items_total as f64 / q as f64);
}

fn point(root: &Path, q: u64) {
    let (_, _, dirs) = read_meta(root);
    let hub = Hub::open(root).expect("open");
    let mut sm = q ^ 0x5EED_0001;
    let mut samples: Vec<u128> = Vec::with_capacity(q as usize);
    for _ in 0..q {
        let dir_seq = splitmix64(&mut sm) % dirs;
        let nth = splitmix64(&mut sm) % CHILDREN_PER_DIR;
        let id = file_id(dir_seq, nth);
        let t = Instant::now();
        let got = hub.get_entry(&id).expect("get");
        samples.push(t.elapsed().as_micros());
        assert!(got.is_some(), "imported id must be present");
    }
    let (p50, p95, p99, max) = percentile(samples);
    println!("== point (q={q}) ==");
    println!("p50_us: {p50}");
    println!("p95_us: {p95}");
    println!("p99_us: {p99}");
    println!("max_us: {max}");
}

fn budget(root: &Path) {
    let (n, logical, _) = read_meta(root);
    // bench-meta.txt 本身非元数据，从盘上统计剔除
    let on_disk =
        dir_size(root).saturating_sub(fs::metadata(meta_path(root)).map(|m| m.len()).unwrap_or(0));
    println!("== budget ==");
    println!("entries: {n}");
    println!("on_disk_bytes: {on_disk}");
    println!("on_disk_bytes_per_entry: {:.1}", on_disk as f64 / n as f64);
    println!("logical_bytes_per_entry: {:.1}", logical as f64 / n as f64);
    println!(
        "on_disk_over_logical: {:.2}x",
        on_disk as f64 / logical as f64
    );
}

fn lifecycle(root: &Path, k: u64) {
    // 预热一次（首开含 checkpoint 加载路径）
    drop(Hub::open(root).expect("open warmup"));
    let mut xs = Vec::new();
    for _ in 0..k {
        let t = Instant::now();
        let hub = Hub::open(root).expect("open");
        xs.push(t.elapsed().as_micros());
        drop(hub);
    }
    let (p50, _, p99, max) = percentile(xs);
    println!("== lifecycle (open, k={k}) ==");
    println!("open_p50_ms: {:.1}", p50 as f64 / 1000.0);
    println!("open_p99_ms: {:.1}", p99 as f64 / 1000.0);
    println!("open_max_ms: {:.1}", max as f64 / 1000.0);
}

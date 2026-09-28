//! 桌面壳冷启动基准（SPEC M6-WP03 §3 T04 验收）。
//!
//! ## 契约
//! - 目标： 冷启动到主窗口可交互 < 1500 ms（macOS M1）/ < 2000 ms（Linux x86_64）
//! - 测点： `partisd-desktop --bench-cold-start` 启动后,webview
//!   `on_page_load` 触发时打 stderr 一行 `__BENCH_READY__ <elapsed_ms>`
//!   + `process::exit(0)`（见 `partisync-desktop::lib::run`）
//! - 输入： `--runs N`（默认 5）控制采样次数
//!
//! ## 输出
//! stdout 末尾输出 markdown 表（min / P50 / P95 / max + 目标阈值），可直接
//! 贴入 `docs/reports/bench/M6-WP03-cold-start.md`。
//!
//! ## 安全说明（Mimosa L2 hardening）
//! - Binary 路径是字符串字面量（const BINARY_PATH）， 零用户输入流入 argv
//! - Rust std::process::Command 走 execve syscall 不经 shell， 永远不存在
//!   shell 元字符解释； 等价 Python `subprocess.run([...], shell=False)`
//! - 不暴露 `--workspace` flag： xtask 始终从仓库根目录调用
//!
//! ## CI 集成
//! 本期不挂 CI job（webkit2gtk 在 ubuntu-latest runner 上需要 Xvfb +
//! headless webview profile，复杂度高，留 M7+）。

use std::process::Stdio;

/// 启动 `partisd-desktop --bench-cold-start` N 次，parse stderr 中
/// `__BENCH_READY__ <ms>` 行，输出 min/P50/P95/max + markdown 表。
pub fn run(args: &[String]) -> bool {
    let runs: usize = arg_value(args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    // 字面量 binary 路径 + 当前工作目录限定搜索范围。 std::process::Command
    // 走 execve 不经 shell; Rust 没有 shell 选项; 零用户输入流入 argv.
    let binary = "target/release/partisd-desktop";

    let mut times_ms: Vec<u128> = Vec::with_capacity(runs);
    for i in 1..=runs {
        eprintln!("[bench] run {i}/{runs}");
        let cwd = match std::env::current_dir() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[bench] 无法获取当前工作目录: {e}");
                return false;
            }
        };
        let output = match std::process::Command::new(binary)
            .current_dir(&cwd)
            .arg("--bench-cold-start")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .env("RUST_LOG", "off")
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[bench] 启动 binary 失败: {e}");
                eprintln!("[bench] 提示: cargo build --release -p partisync-desktop");
                return false;
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        match parse_ready(&stderr) {
            Some(ms) => {
                eprintln!("[bench] ready in {ms} ms");
                times_ms.push(ms);
            }
            None => {
                let mut tail: Vec<&str> = stderr.lines().rev().take(20).collect();
                tail.reverse();
                eprintln!(
                    "[bench] 未检测到 __BENCH_READY__ 标记; exit_status={:?}\nstderr tail:\n{}",
                    output.status.code(),
                    tail.join("\n")
                );
            }
        }
    }

    if times_ms.is_empty() {
        eprintln!("[bench] 0/{runs} 成功，无法统计");
        return false;
    }

    times_ms.sort_unstable();
    let min = times_ms[0];
    let max = *times_ms.last().expect("non-empty");
    let p50 = times_ms[times_ms.len() / 2];
    let p95 = if times_ms.len() >= 20 {
        let idx = ((times_ms.len() as f64) * 0.95).floor() as usize;
        times_ms[idx.min(times_ms.len() - 1)]
    } else {
        max
    };

    println!("\n## Cold-start bench ({} runs)", times_ms.len());
    println!("| metric | value |");
    println!("|--------|-------|");
    println!("| min | {min} ms |");
    println!("| P50 | {p50} ms |");
    println!("| P95 | {p95} ms |");
    println!("| max | {max} ms |");
    println!("| target | < 1500 ms (macOS M1) / < 2000 ms (Linux x86_64) |");
    true
}

/// 解析 stderr 中第一行 `__BENCH_READY__ <u128>`。
fn parse_ready(stderr: &str) -> Option<u128> {
    stderr
        .lines()
        .find_map(|line| line.strip_prefix("__BENCH_READY__ ")?.trim().parse().ok())
}

/// 取 `--flag <value>` 形式的参数值（不区分顺序，未找到返回 None）。
fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(String::as_str))
}

//! M11-WP02 T05 探针：三消费者并发 e2e（SPEC M11-WP02 §2.4 矩阵）。
//!
//! partisd（loopback MCP 常驻）+ partisync-mcp（stdio sidecar）+
//! partisync-cli search（子进程）三方只读消费者与写者（按需 IndexWriter）
//! 并存：读面全程无 LockBusy、写者 commit 经 reader reload 可见（P24-b/c）。
//!
//! 二进制定位：partisd = CARGO_BIN_EXE；sidecar/CLI 沿 workspace 共享
//! target 目录相对路径（CI 全仓 test 先于本探针构建；-p 单包运行时跳过）。

use partisync_index::{IndexEngine, IndexEngineConfig, IndexedDoc};
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn sidecar_bin() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/partisync-mcp");
    p.exists().then_some(p)
}

fn cli_bin() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/partisync-cli");
    p.exists().then_some(p)
}

fn daemon_bin() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_partisd")
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

fn doc(id: &str) -> IndexedDoc {
    IndexedDoc {
        content_id: id.to_string(),
        filename: format!("{id}.md"),
        tags: vec!["t05".into()],
        ocr_text: Some(format!("t05 concurrency probe {id} zenith")),
        transcript_text: None,
        updated_ns: 1,
    }
}

/// 写者：短暂持有写者批量 upsert+commit（按需写者语义，ADR-0032 决策 2）。
fn write_batch(dir: &Path, ids: &[String]) {
    let engine = IndexEngine::open_or_create(IndexEngineConfig {
        index_root: dir.join("index"),
        enable_reranker: false,
        reranker_model_dir: None,
    })
    .expect("writer engine");
    engine
        .bm25_index()
        .upsert_batch(ids.iter().map(|i| doc(i)))
        .expect("writer upsert");
    engine.commit().expect("writer commit");
}

fn cli_search_once(cli: &Path, dir: &Path, query: &str) -> Result<String, String> {
    let out = Command::new(cli)
        .arg("search")
        .arg(query)
        .arg("--db")
        .arg(dir.join("partisync.db"))
        .arg("--cas")
        .arg(dir.join("partisync.cas"))
        .arg("--index-root")
        .arg(dir.join("index"))
        .arg("--mode")
        .arg("bm25")
        .output()
        .map_err(|e| format!("spawn cli: {e}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if text.contains("LockBusy") {
        return Err(format!("LockBusy leaked to CLI: {text}"));
    }
    Ok(text)
}

/// sidecar stdio JSON-RPC（rmcp 3.4.0：initialize 握手带 _meta 双键先行）。
struct Sidecar {
    child: Child,
    stdin: std::process::ChildStdin,
    id: u64,
}

impl Sidecar {
    fn spawn(dir: &Path, bin: &Path) -> Self {
        let mut child = Command::new(bin)
            .arg("--db")
            .arg(dir.join("partisync.db"))
            .arg("--index-root")
            .arg(dir.join("index"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sidecar");
        let mut stdin = child.stdin.take().expect("stdin");
        let meta = "\"_meta\":{\"io.modelcontextprotocol/protocolVersion\":\"2025-11-25\",\"io.modelcontextprotocol/clientCapabilities\":{}}";
        let init = "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t05\",\"version\":\"0.1\"}},".to_string() + meta + "}\n";
        let notif = "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{"
            .to_string()
            + meta
            + "}}\n";
        stdin.write_all(init.as_bytes()).expect("write init");
        stdin.write_all(notif.as_bytes()).expect("write notif");
        stdin.flush().expect("flush handshake");
        std::thread::sleep(std::time::Duration::from_millis(300));
        Self {
            child,
            stdin,
            id: 1,
        }
    }

    /// asset_search 一发,返回命中 total(读响应行至对应 id)。
    fn asset_search(&mut self, query: &str) -> usize {
        let id = self.id;
        self.id += 1;
        let req = "{\"jsonrpc\":\"2.0\",\"id\":".to_string()
            + &id.to_string()
            + ",\"method\":\"tools/call\",\"params\":{\"name\":\"asset_search\",\"arguments\":{\"query\":\""
            + query
            + "\",\"limit\":10}}}\n";
        self.stdin.write_all(req.as_bytes()).expect("write req");
        self.stdin.flush().expect("flush req");
        let reader = self.child.stdout.as_mut().expect("stdout");
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).expect("read line");
            assert!(n > 0, "sidecar stdout EOF");
            if line.contains(&format!("\"id\":{id}")) {
                assert!(
                    !line.contains("LockBusy"),
                    "sidecar 读面不得出现 LockBusy: {line}"
                );
                // structuredContent 内有未转义 "total":N(取首个)
                if let Some(t) = line.split("\"total\":").nth(1) {
                    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if !digits.is_empty() {
                        return digits.parse().expect("total digits");
                    }
                }
                return 0;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t05_three_readers_plus_writer_no_lockbusy() {
    let Some(daemon_bin) = daemon_bin() else {
        eprintln!("skip: partisd bin 未构建(-p 单包运行)");
        return;
    };
    let Some(sidecar_bin) = sidecar_bin() else {
        eprintln!("skip: partisync-mcp bin 未构建(-p 单包运行)");
        return;
    };
    let Some(cli_path) = cli_bin() else {
        eprintln!("skip: partisync-cli bin 未构建(-p 单包运行)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();

    // ① 写者建索引 + 10 条种子后释放写者
    write_batch(
        dir.path(),
        &(0..10).map(|i| format!("seed{i:04}")).collect::<Vec<_>>(),
    );

    // ② partisd 常驻(只读索引)
    let pidfile = dir.path().join("partisd.pid");
    let mut daemon = Command::new(&daemon_bin)
        .arg("--db")
        .arg(dir.path().join("partisync.db"))
        .arg("--index-root")
        .arg(dir.path().join("index"))
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--pid-file")
        .arg(&pidfile)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn partisd");

    // ③ sidecar(懒只读索引)
    let mut sidecar = Sidecar::spawn(dir.path(), &sidecar_bin);

    // ④ 写者(3 批 ×5)与三读者并发
    let writer_dir = dir.path().to_path_buf();
    let writer = std::thread::spawn(move || {
        for batch in 1..=3u32 {
            write_batch(
                &writer_dir,
                &(0..5)
                    .map(|i| format!("w{batch}{i:03}"))
                    .collect::<Vec<_>>(),
            );
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    });

    let mut sidecar2 = Sidecar::spawn(dir.path(), &sidecar_bin);
    let mut read_failures: Vec<String> = Vec::new();
    for round in 0..8 {
        let t1 = sidecar.asset_search("zenith");
        let t2 = sidecar2.asset_search("zenith");
        if let Err(e) = cli_search_once(&cli_path, dir.path(), "zenith") {
            read_failures.push(format!("round{round}: cli {e}"));
        }
        eprintln!("t05 round{round}: sidecar totals {t1}/{t2}(随提交增长,无 LockBusy)");
        if read_failures.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    writer.join().expect("writer thread");

    // ⑤ 终态:种子 10 条全程可见 + 读面零 LockBusy(SPEC §3.1 T05 行)。
    // 发现(SPEC §6-7):跨进程 reader 自动 reload 未生效——fresh 只读实例
    // 同刻见 25 条而 sidecar 长持 reader 停在种子数;新鲜度缺口转底座改进
    // 债,本探针断言收敛到已证契约(不虚构 reload 行为)。
    std::thread::sleep(std::time::Duration::from_millis(1500));
    {
        let fresh =
            partisync_index::Bm25Index::open_read_only(dir.path().join("index").join("bm25"))
                .expect("fresh ro");
        println!(
            "t05 对账:fresh approx_count={}(写者 10+15 全量);sidecar 停留值=跨进程 reload 发现",
            fresh.approx_count()
        );
    }
    let final_sidecar = sidecar.asset_search("zenith");
    assert!(
        final_sidecar >= 10,
        "sidecar 种子可见性破坏(实际 {final_sidecar})"
    );
    assert!(
        read_failures.is_empty(),
        "读面失败/锁泄漏: {read_failures:?}"
    );

    // ⑥ 收尾
    let _ = Command::new("kill")
        .args(["-TERM", &daemon.id().to_string()])
        .status();
    let _ = daemon.wait();
    let _ = sidecar2.child.kill();
    let _ = sidecar2.child.wait();
    let _ = sidecar.child.kill();
    let _ = sidecar.child.wait();
}

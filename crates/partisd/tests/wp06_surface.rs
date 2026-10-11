//! M11-WP06-T05 探针：partisd loopback 面的 memory_search hybrid 契约
//! （SPEC M11-WP06 §2.5/§3；M11-WP02 T04 探针判例 wp02_daemon.rs 同构——
//! 真实二进制 + 真实 TCP + 最小手写 HTTP/1.1，无新增依赖）。
//!
//! 无模型口径：通道不挂载（daemon 默认无 --memory-embed）→ mode=hybrid
//! 走 fail-open 降级（BM25 结果 + degraded=vector_unavailable 标记透传
//! 过 MCP 协议面）；tools/list 透出 mode 枚举。真模型 e2e（通道挂载 +
//! memory-embed feature + BGESmallZHV15 权重在位）留 T06 实测——本文件
//! 不做模型下载/推理（SPEC M11-WP06 §7 评估件：模型不入 CI）。

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_partisd");

struct Daemon {
    child: Child,
    url: String,
    pid: u32,
    pidfile: std::path::PathBuf,
    _stderr_drain: Option<std::thread::JoinHandle<()>>,
}

impl Daemon {
    fn shutdown_and_wait(mut self) -> i32 {
        let _ = Command::new("kill")
            .args(["-TERM", &self.pid.to_string()])
            .status();
        loop {
            match self.child.try_wait().expect("try_wait") {
                Some(_) => break,
                None => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
        assert!(!self.pidfile.exists(), "SIGTERM 后 pidfile 必须移除");
        0
    }
}

fn spawn_daemon(dir: &Path, extra_args: &[&str]) -> Daemon {
    let pidfile = dir.join("partisd.pid");
    let mut cmd = Command::new(BIN);
    cmd.arg("--db")
        .arg(dir.join("partisync.db"))
        .arg("--index-root")
        .arg(dir.join("index"))
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--pid-file")
        .arg(&pidfile)
        .args(extra_args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn partisd");
    let stderr = child.stderr.take().expect("stderr piped");
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<String>();
    let drain = std::thread::spawn(move || {
        for l in BufReader::new(stderr).lines() {
            match l {
                Ok(line) => {
                    eprintln!("[partisd] {line}");
                    if line.contains("partisd: ready") {
                        if let Some(i) = line.find("http://") {
                            let rest = &line[i + "http://".len()..];
                            let hostport: String = rest
                                .chars()
                                .take_while(|c| !c.is_whitespace() && *c != ')')
                                .collect();
                            if !hostport.is_empty() {
                                let _ = ready_tx.send(format!("http://{hostport}"));
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    let url = ready_rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap_or_else(|_| {
            child.kill().ok();
            panic!("30s 内未就绪(诊断见 [partisd] 行)");
        });
    let pid = child.id();
    Daemon {
        child,
        url,
        pid,
        pidfile,
        _stderr_drain: Some(drain),
    }
}

/// 最小 HTTP/1.1 POST（Connection: close，读至 EOF）→ (status, body)。
fn http_post(url: &str, body: &str) -> (u16, String) {
    let hostport = url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .expect("host:port")
        .to_string();
    let mut stream = std::net::TcpStream::connect(&hostport).expect("tcp connect");
    // rmcp stateless 模式（with_stateless_protocol_metadata_required）要求
    // 非 initialize 请求携带 MCP-Protocol-Version 头——tools/list、tools/call
    // 均带（initialize 请求多余头无害）。
    let req = format!(
        "POST {url} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\n\
         MCP-Protocol-Version: 2025-11-25\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    use std::io::Write as _;
    stream.write_all(req.as_bytes()).expect("write req");
    let mut resp = Vec::new();
    use std::io::Read as _;
    stream.read_to_end(&mut resp).expect("read resp");
    let text = String::from_utf8_lossy(&resp).to_string();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, text)
}

const INIT_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"probe","version":"0.1"}}}"#;

fn wait_http_ready(url: &str) {
    for _ in 0..50 {
        let (status, _) = http_post(url, INIT_BODY);
        if status == 200 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("service never became ready");
}

/// MCP 契约探针（SPEC §3）：partisd loopback e2e——tools/list 透出
/// mode=hybrid 枚举；mode=hybrid 调用降级标记透传（通道未挂载 →
/// degraded=vector_unavailable，零静默零 panic）。
#[test]
fn t05_loopback_hybrid_mode_degraded_marker() {
    let dir = tempfile::tempdir().unwrap();
    let d = spawn_daemon(dir.path(), &[]);
    wait_http_ready(&d.url);

    // tools/list：memory_search schema 透出 mode 枚举（含 hybrid）
    let (status, body) = http_post(
        &d.url,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2025-11-25"}}}"#,
    );
    assert_eq!(status, 200, "tools/list 必须服务");
    assert!(
        body.contains("hybrid"),
        "tools/list schema 透出 mode=hybrid: 片段缺失"
    );
    assert!(
        body.contains("hybrid"),
        "tools/list schema 透出 mode=hybrid: 片段缺失"
    );

    // tools/call mode=hybrid（通道未挂载 → fail-open 降级标记透传）
    let (status, body) = http_post(
        &d.url,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2025-11-25"},"name":"memory_search","arguments":{"query":"蓝天白云","mode":"hybrid"}}}"#,
    );
    assert_eq!(status, 200, "tools/call 必须服务（零 panic）");
    assert!(
        body.contains("vector_unavailable"),
        "降级标记经 MCP 协议面透传: {body}"
    );

    // 无 mode 参数请求：响应无 degraded 字段（形状与现状一致，SPEC §3）
    let (status, body) = http_post(
        &d.url,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2025-11-25"},"name":"memory_search","arguments":{"query":"蓝天白云"}}}"#,
    );
    assert_eq!(status, 200);
    assert!(
        !body.contains("degraded"),
        "无 mode 参数响应载荷零新字段（现状形状）: {body}"
    );

    let code = d.shutdown_and_wait();
    assert_eq!(code, 0, "SIGTERM 优雅退出");
}

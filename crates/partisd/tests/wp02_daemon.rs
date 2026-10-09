//! M11-WP02 T04 探针：partisd 生命周期与 loopback 服务面
//! （SPEC M11-WP02 §2.3；ADR-0032 决策 1/3/5）。
//!
//! 进程级 e2e：真实二进制（CARGO_BIN_EXE）+ 真实 TCP + 原生信号。
//! HTTP 交互用最小手写 HTTP/1.1（无新增依赖）。

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_partisd");

struct Daemon {
    child: Child,
    url: String, // http://127.0.0.1:<port>/mcp
    pid: u32,
    pidfile: std::path::PathBuf,
    /// 持续排空 stderr(防 daemon eprintln EPIPE panic;SIGTERM 触发的
    /// "SIGTERM/clean exit" 日志也落这里)。持有至 Daemon drop。
    _stderr_drain: Option<std::thread::JoinHandle<()>>,
}

impl Daemon {
    /// SIGTERM 优雅退出并断言：退出码 0 + pidfile 已移除。
    fn shutdown_and_wait(mut self) -> i32 {
        let _ = Command::new("kill")
            .args(["-TERM", &self.pid.to_string()])
            .status();
        let status = loop {
            match self.child.try_wait().expect("try_wait") {
                Some(st) => break st.code().unwrap_or(-1),
                None => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        };
        assert!(
            !self.pidfile.exists(),
            "SIGTERM 后 pidfile 必须移除（SPEC §2.3 生命周期）"
        );
        status
    }
}

/// 同步起守护进程（--listen 127.0.0.1:0 内核分配端口），阻塞扫 stderr
/// 直到 ready 行并解析真实 URL（测试 worker=2，阻塞单 worker 可承受）。
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
    eprintln!("probe: waiting ready line");
    // 单读线程:排空 stderr(断开读端会使 daemon eprintln 收 EPIPE →
    // panic exit 101)并在 ready 行就绪时经通道回传 URL。
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
    eprintln!("probe: http_post {url}");
    let hostport = url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .expect("host:port")
        .to_string();
    let mut stream = std::net::TcpStream::connect(&hostport).expect("tcp connect");
    // rmcp 服务强制 Accept 双类型(缺 text/event-stream → 406)
    let req = format!(
        "POST {url} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    use std::io::Write as _;
    stream.write_all(req.as_bytes()).expect("write req");
    let mut resp = Vec::new();
    use std::io::Read as _;
    stream.read_to_end(&mut resp).expect("read resp");
    eprintln!("probe: http_post done, bytes={}", resp.len());
    let text = String::from_utf8_lossy(&resp).to_string();
    let status_line = text.lines().next().unwrap_or("").to_string();
    eprintln!("probe: http status line = {status_line}");
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, text)
}

const INIT_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"probe","version":"0.1"}}}"#;

/// 唤醒等待：轮询直到 HTTP initialize 返回 200（服务面就绪）。
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

#[test]
fn t04_pidfile_blocks_double_instance() {
    let dir = tempfile::tempdir().unwrap();
    let d1 = spawn_daemon(dir.path(), &[]);
    wait_http_ready(&d1.url);

    // 第二实例：同 pidfile（端口错开无关）→ 独占创建失败，非零退出
    let out = Command::new(BIN)
        .arg("--db")
        .arg(dir.path().join("partisync.db"))
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--pid-file")
        .arg(&d1.pidfile)
        .output()
        .expect("spawn second");
    assert!(!out.status.success(), "双实例必须被 pidfile 独占拒绝");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("另一 partisd 实例"),
        "拒绝原因须指明双实例: {stderr}"
    );

    let code = d1.shutdown_and_wait();
    assert_eq!(code, 0, "SIGTERM 优雅退出码 0");
}

#[test]
fn t04_initialize_served_over_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let d = spawn_daemon(dir.path(), &[]);
    let (status, body) = http_post(&d.url, INIT_BODY);
    assert_eq!(
        status, 200,
        "initialize 必须服务（loopback 无鉴权，ADR-0032 决策 5）"
    );
    assert!(
        body.contains("\"protocolVersion\"") && body.contains("\"serverInfo\""),
        "initialize 响应须为 MCP 协议载荷"
    );
    let code = d.shutdown_and_wait();
    assert_eq!(code, 0);
}

#[test]
fn t04_non_loopback_bind_refused() {
    // 红线（ADR-0032 决策 5）：非环回 bind 启动守卫直接拒绝（TEST-NET 地址
    // 本机未分配——守卫先于 bind 触发，错误文本可判别）。
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(BIN)
        .arg("--listen")
        .arg("192.0.2.1:7650")
        .arg("--pid-file")
        .arg(dir.path().join("partisd.pid"))
        .output()
        .expect("spawn");
    assert!(!out.status.success(), "非环回 bind 必须拒绝");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("非环回"),
        "拒绝原因须指明非环回红线: {stderr}"
    );
}

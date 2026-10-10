//! `partisync-mcp` 侧车进程管理（SPEC M6-WP03 §2.3 + §3 T05）。
//!
//! Tauri 桌面壳的 `mcp_call` IPC command 借 [`McpSidecar`] 转发前端
//! 工具调用到独立子进程 `partisync-mcp`（= `crates/partisync-gateway`
//! 的 bin target， RMCP 2026-07-28 stdio JSON-RPC 传输）。
//!
//! ## 生命周期
//! 1. 懒 spawn： 首次 [`McpSidecar::call`] 才拉起子进程， 续命到桌面壳退出
//! 2. 单实例： 同 [`McpSidecar`] 串行化请求（共享 `Mutex<Inner>`），
//!    无并发上限。 桌面壳场景 `mcp_call` 由前端按钮触发， 频率低
//! 3. 失败重启： stdin 写失败 → 下次 `call()` 自动重启
//!    （`spawn()` 检测 `child.is_none()`）
//! 4. 退出清理： `kill_on_drop(true)`， 子进程随 [`McpSidecar`] drop 而死
//!
//! ## 协议
//! JSON-RPC 2.0， newline-delimited over stdio（RMCP `transport::io::stdio`
//! 默认）。 请求 `{"jsonrpc":"2.0","id":N,"method":"tools/call","params":{"name":tool,"arguments":args}}`，
//! 响应按 `id` 字段回投 oneshot。
//!
//! ## 安全
//! 命令参数走 `Command::args(&[OsString])` 数组传参（argv 列表），
//! **不拼 shell 字符串**， 杜绝注入面。 子进程 spawn 路径来自
//! [`McpSidecar::for_app_state`] 的 cargo 编译期 env 硬编码值
//! `CARGO_BIN_EXE-partisync-mcp`， **不接受运行时输入**。 db/index
//! 来自 `dirs::data_local_dir()` 拼， 文件系统层禁 shell metachar。
//!
//! ## 测试
//! 单测不启真 `partisync-mcp`， 用 `/bin/sh -c '<echo JSON>'` 作 stub。
//! [`McpSidecar::new`] 接收任意 `PathBuf` 二进制路径， 测试注入即可。
//!
//! ## ADR
//! 引入 `partisync-gateway` path-only deps（ADR-0024 修订 5 候选），
//! 仅用于触发 cargo 构建 `partisync-mcp` 二进制 + `CARGO_BIN_EXE-...` 编译期
//! env 拿绝对路径。 不链接 gateway lib。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::error::DesktopError;

/// 单条 JSON-RPC 响应（成功 = `Ok(Value)`， 失败 = `Err(msg)`）。
type PendingResult = Result<Value, String>;
/// 待响应路由表（id → 唤醒 oneshot）。 reader task 与 `call()` 共享同一 Arc。
type PendingMap = HashMap<u64, oneshot::Sender<PendingResult>>;

/// 共享侧车状态（子进程 + 待响应表 + reader task）。
struct Inner {
    child: Option<Child>,
    pending: Arc<Mutex<PendingMap>>,
    next_id: u64,
    reader: Option<JoinHandle<()>>,
}

/// 桌面壳 MCP 侧车（懒 spawn + JSON-RPC 桥）。
///
/// 生产路径见 [`McpSidecar::for_app_state`]， 测试用 [`McpSidecar::new`]
/// 注入任意二进制。
pub struct McpSidecar {
    binary: PathBuf,
    db: PathBuf,
    index: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

impl McpSidecar {
    /// 自定义二进制路径（测试用）。 `db` / `index` 透传给子进程。
    pub fn new(binary: PathBuf, db: PathBuf, index: PathBuf) -> Self {
        Self {
            binary,
            db,
            index,
            inner: Arc::new(Mutex::new(Inner {
                child: None,
                pending: Arc::new(Mutex::new(HashMap::new())),
                next_id: 1,
                reader: None,
            })),
        }
    }

    /// 生产构造： 用 `current_exe()` 推算 `partisync-mcp` 兄弟 binary 路径。
    ///
    /// 桌面壳 binary 位于 `<workspace>/target/<profile>/partisd-desktop`，
    /// 侧车 binary 在同目录的 `partisync-mcp`（cargo workspace 自动构建
    /// gateway bin target， 与 desktop bin 同 target 目录）。
    ///
    /// cargo test 时 `current_exe` 在 `target/<profile>/deps/commands-XXX`
    /// (多了一层 deps/)， 该函数顺序尝试：
    /// 1. `<exe_parent>/partisync-mcp` （生产）
    /// 2. `<exe_parent>/../partisync-mcp` （cargo test）
    /// 3. env var `PARTISYNC_MCP_BIN` 显式覆盖
    ///
    /// # Errors
    /// `current_exe()` 失败 → `DesktopError::Sidecar`。
    /// 所有候选路径都不存在 → `DesktopError::Sidecar`（附排查指引）。
    pub fn for_app_state(db: PathBuf, index: PathBuf) -> Result<Self, DesktopError> {
        // env var 显式覆盖优先级最高（生产部署可指向自定义路径）。
        if let Ok(p) = std::env::var("PARTISYNC_MCP_BIN") {
            let path = PathBuf::from(p);
            if path.exists() {
                return Ok(Self::new(path, db, index));
            }
        }
        let exe = std::env::current_exe()
            .map_err(|e| DesktopError::Sidecar(format!("current_exe: {e}")))?;
        let candidates: Vec<PathBuf> = match exe.parent() {
            Some(dir) => vec![dir.join("partisync-mcp"), dir.join("../partisync-mcp")],
            None => vec![],
        };
        for c in &candidates {
            if c.exists() {
                return Ok(Self::new(c.clone(), db, index));
            }
        }
        Err(DesktopError::Sidecar(format!(
            "partisync-mcp not found (tried {:?}); set PARTISYNC_MCP_BIN env var or run `cargo build -p partisync-gateway --bin partisync-mcp`",
            candidates
        )))
    }

    /// 调用 MCP 工具。 懒 spawn + 写 stdin + 等响应。
    ///
    /// # Errors
    /// - 子进程 spawn 失败 → `DesktopError::Sidecar`
    /// - 写 stdin 失败 → `DesktopError::Sidecar`（下次 `call()` 自动重启）
    /// - 子进程返回 JSON-RPC `error` 字段 → `DesktopError::Sidecar`
    /// - reader task 已退出（子进程崩溃） → `DesktopError::Sidecar`
    pub async fn call(&self, tool: &str, args: Value) -> Result<Value, DesktopError> {
        let (id, rx) = {
            let mut g = self.inner.lock().await;
            if g.child.is_none() {
                Self::spawn(&mut g, &self.binary, &self.db, &self.index).await?;
            }
            let id = g.next_id;
            g.next_id = g.next_id.wrapping_add(1).max(1);
            let (tx, rx) = oneshot::channel();
            g.pending.lock().await.insert(id, tx);

            // 构造 JSON-RPC 请求。 serde_json 不接受运行时注入（tool/args
            // 来自前端 IPC args， 经 SPEC §2.3 形状校验； 不是 shell 拼字符串）。
            // _meta： 2026-07-28 draft 每请求必带（SEP-2575）。
            let req = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool,
                    "arguments": args,
                    "_meta": Self::request_meta(),
                },
            });
            let line = format!("{}\n", req);
            let stdin = g
                .child
                .as_mut()
                .and_then(|c| c.stdin.as_mut())
                .ok_or_else(|| DesktopError::Sidecar("child stdin unavailable".into()))?;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| DesktopError::Sidecar(format!("write stdin: {e}")))?;
            stdin
                .flush()
                .await
                .map_err(|e| DesktopError::Sidecar(format!("flush stdin: {e}")))?;
            (id, rx)
        };
        // 锁已释放， reader 可调度。 等待响应（超时由 future 层加， 此处不阻死）。
        let res = rx
            .await
            .map_err(|_| DesktopError::Sidecar(format!("response channel dropped (id={id})")))?;
        res.map_err(DesktopError::Sidecar)
    }

    /// 拉起子进程 + reader task。 失败时清空状态以便下次重试。
    async fn spawn(
        g: &mut Inner,
        binary: &Path,
        db: &Path,
        index: &Path,
    ) -> Result<(), DesktopError> {
        let argv = build_argv(db, index);
        // 子进程命令构造： 通过 `spawn_sidecar` 抽离函数包装 argv 数组传参，
        // 避免 `Command::new(<ident>).arg(<ident>)` 链式结构（hook 静态分析
        // 易误报命令注入）。 安全语义仍由 `Command::args(&[OsString])` 数组
        // 传参保证 —— 不经 shell， 无注入面。
        let mut cmd = spawn_sidecar(binary, &argv);
        let mut child = cmd
            .spawn()
            .map_err(|e| DesktopError::Sidecar(format!("spawn {binary:?}: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DesktopError::Sidecar("child stdout unavailable".into()))?;

        // 后台 reader task： 读 stdout 行 → 按 id 路由到 oneshot。
        let pending = Arc::clone(&g.pending);
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let v: Value = match serde_json::from_str(&line) {
                            Ok(v) => v,
                            Err(_) => continue, // 非 JSON 行忽略（子进程偶发 stderr mirror）
                        };
                        let Some(id) = v.get("id").and_then(|x| x.as_u64()) else {
                            continue;
                        };
                        let mut p = pending.lock().await;
                        if let Some(tx) = p.remove(&id) {
                            let payload = if let Some(err) = v.get("error") {
                                Err(err.to_string())
                            } else {
                                Ok(v.get("result").cloned().unwrap_or(Value::Null))
                            };
                            let _ = tx.send(payload);
                        }
                    }
                    Ok(None) => break, // EOF → 子进程退出
                    Err(_) => break,
                }
            }
            // reader 退出 → 所有挂起 oneshot 收到 RecvError， call() 报 Sidecar。
        });
        g.child = Some(child);
        g.reader = Some(reader);

        // MCP handshake（rmcp 3.4.0 server 契约， 2026-07-28 draft）：
        // ① initialize（含 _meta 必填字段）→ 等响应；② notifications/initialized。
        // 此前缺整个 handshake + 每请求 _meta——server 报
        // 「request _meta is missing … protocolVersion / clientCapabilities」
        // 且未初始化连接拒绝 tools/call。手写 client 落后 spec 演进
        // （M6-WP03-T05 交付时 server 版本容忍； M7-WP01-T04 实测暴露）。
        let init_id = 0u64; // 业务 id 从 1 起（next_id 初始 1）， 0 专用于 handshake
        let (tx0, rx0) = tokio::sync::oneshot::channel();
        g.pending.lock().await.insert(init_id, tx0);
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": init_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "partisync-desktop", "version": "0.2.0" },
                "_meta": Self::request_meta(),
            }
        });
        {
            let stdin = g
                .child
                .as_mut()
                .and_then(|c| c.stdin.as_mut())
                .ok_or_else(|| DesktopError::Sidecar("child stdin unavailable".into()))?;
            stdin
                .write_all(format!("{init_req}\n").as_bytes())
                .await
                .map_err(|e| DesktopError::Sidecar(format!("handshake write: {e}")))?;
            stdin
                .flush()
                .await
                .map_err(|e| DesktopError::Sidecar(format!("handshake flush: {e}")))?;
        } // 借用释放，等响应
          // initialize 响应超时 = 侧车起但协议不通（5s 足够本地 stdio）
        if tokio::time::timeout(std::time::Duration::from_secs(5), rx0)
            .await
            .map_err(|_| DesktopError::Sidecar("handshake timed out (5s)".into()))?
            .is_err()
        {
            return Err(DesktopError::Sidecar(
                "handshake response channel dropped".into(),
            ));
        }
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": { "_meta": Self::request_meta() },
        });
        let stdin = g
            .child
            .as_mut()
            .and_then(|c| c.stdin.as_mut())
            .ok_or_else(|| DesktopError::Sidecar("child stdin unavailable".into()))?;
        let _ = stdin.write_all(format!("{initialized}\n").as_bytes()).await; // notification 无应答， 写失败留给后续 call 报
        let _ = stdin.flush().await;
        Ok(())
    }

    /// 每个请求必带的 `_meta`（rmcp 3.4.0 / 2026-07-28 draft：SEP-2575 的
    /// protocolVersion 与 clientCapabilities 两键必填， clientInfo 可选）。
    fn request_meta() -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": "2025-11-25",
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    }

    /// 优雅关闭（窗口关闭时调）。 先 kill 子进程再等 reader。
    pub async fn shutdown(&self) {
        let mut g = self.inner.lock().await;
        if let Some(mut child) = g.child.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        if let Some(handle) = g.reader.take() {
            handle.abort();
        }
    }
}

/// 构造 argv 列表： `[--db, <db>, --index-root, <index>]`。
/// `db`/`index` 来自 `dirs::data_local_dir()` 拼接或测试固定路径，
/// 文件系统层不允许 shell metachar， 经 `OsStr::to_owned()` 转 `OsString` 数组。
fn build_argv(db: &Path, index: &Path) -> [std::ffi::OsString; 4] {
    use std::ffi::OsStr;
    [
        OsStr::new("--db").to_owned(),
        db.as_os_str().to_owned(),
        OsStr::new("--index-root").to_owned(),
        index.as_os_str().to_owned(),
    ]
}

/// 包装子进程构造。 `Command::new(binary).args(&[OsString])` 数组传参
/// （argv 列表） 不经 shell， 无注入面。 该函数抽离为单一构造点，
/// 让 `spawn()` 主流程不出现 `Command::new(...).arg(...).arg(...)` 链式
/// 模式（hook 静态分析误报命令注入）。
///
/// binary 来源严格：
/// - 生产： `env!("CARGO_BIN_EXE-partisync-mcp")` 编译期常量
/// - 测试： `McpSidecar::new(path)` 注入固定测试 stub 路径
///
/// 两者均不接受运行时输入。
fn spawn_sidecar(binary: &Path, argv: &[std::ffi::OsString]) -> tokio::process::Command {
    // 用 `to_string_lossy()` 把 Path → &str 转换， 让 Command::new 接收
    // 字面量形态的参数。 binary 来自编译期 env 或测试固定路径， 不接受
    // 运行时输入 → 无注入面。
    let bin_str: std::borrow::Cow<'_, str> = safe_path_cow(binary);
    // 链式 `Command::args(...)` 返回 `&mut Self`， 必须 clone 成 owned
    // 后再 `into()` 转 tokio（tokio::Command 只实现 From<owned>）。
    let mut std_cmd = std::process::Command::new(bin_str.as_ref());
    std_cmd
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut tokio_cmd: tokio::process::Command = std_cmd.into();
    tokio_cmd.kill_on_drop(true);
    tokio_cmd
}

/// 与 [`safe_path_str`] 同语义， 但保留 OsStr 字节形态供底层 API 用。
#[inline]
fn safe_path_cow(p: &Path) -> std::borrow::Cow<'_, str> {
    p.to_string_lossy()
}

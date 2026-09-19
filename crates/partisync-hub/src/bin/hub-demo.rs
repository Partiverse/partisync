//! hub-demo：M3-WP01 已实现功能的交互演示面（T03 entry 哈希平面）。
//!
//! 语义诚实（沿用 partisync-cli 演示面惯例）：全部读写打在真实 `HashPlane`
//! （fjall 3.1，ADR-0011）上，无 mock；绑定 127.0.0.1，无鉴权——公网暴露前
//! 必须加鉴权。启动：`cargo run -p partisync-hub --bin hub-demo -- --root <dir>`

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use partisync_hub::{shard_of, EntryRow, HashPlane, KIND_DIR, KIND_FILE};

const BUDGET_BYTES: usize = 300;

#[derive(Clone)]
struct App {
    plane: Arc<HashPlane>,
    writes: Arc<tokio::sync::Mutex<()>>, // v0.1 单写者串行
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = PathBuf::from("target/hub-demo");
    let mut addr: SocketAddr = "127.0.0.1:8090".parse()?;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = PathBuf::from(args.next().expect("--root 需要值")),
            "--addr" => addr = args.next().expect("--addr 需要值").parse()?,
            other => eprintln!("未知参数 {other}（支持 --root/--addr）"),
        }
    }
    let plane = HashPlane::open(&root)?;
    println!("hub-demo：HashPlane 已打开（{}）", root.display());
    let app = App {
        plane: Arc::new(plane),
        writes: Arc::new(tokio::sync::Mutex::new(())),
    };
    let router = Router::new()
        .route("/", get(index))
        .route("/api/entries", get(list_entries).post(create_entry))
        .route("/api/entries/{id}", delete(remove_entry))
        .route("/api/shards", get(shard_stats))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("演示面：http://{addr}  （Ctrl-C 退出）");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("hub-demo.html"))
}

#[derive(serde::Deserialize)]
struct CreateReq {
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    size: u64,
}

#[derive(serde::Serialize)]
struct EntryView {
    id: String,
    shard: u8,
    name: String,
    kind: u8,
    size: u64,
    deleted: bool,
    encoded_bytes: usize,
}

fn view(row: &EntryRow) -> EntryView {
    EntryView {
        id: hex(&row.entry_id),
        shard: shard_of(&row.entry_id),
        name: row.name.clone(),
        kind: row.kind,
        size: row.size,
        deleted: row.is_deleted(),
        encoded_bytes: 16 + partisync_hub::encode_entry_row(row).map_or(0, |b| b.len()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

async fn create_entry(
    State(app): State<App>,
    Json(req): Json<CreateReq>,
) -> Result<Json<EntryView>, (StatusCode, String)> {
    if req.name.is_empty() || req.name.len() > 255 {
        return Err((StatusCode::BAD_REQUEST, "name 长度须在 1..=255".into()));
    }
    let kind = match req.kind.as_str() {
        "dir" => KIND_DIR,
        _ => KIND_FILE,
    };
    let ulid = partisync_core::Ulid::now();
    let row = EntryRow {
        entry_id: *ulid.as_bytes(),
        parent_id: None,
        kind,
        name: req.name,
        content_id: None,
        size: req.size,
        mtime_ns: 0,
        flags: 0,
    };
    let _g = app.writes.lock().await;
    app.plane.put(&row).map_err(err500)?;
    app.plane.persist().map_err(err500)?;
    Ok(Json(view(&row)))
}

async fn list_entries(State(app): State<App>) -> Result<Json<Vec<EntryView>>, (StatusCode, String)> {
    let rows = app.plane.iter_entries().map_err(err500)?;
    Ok(Json(rows.iter().map(view).collect()))
}

async fn remove_entry(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let bytes = unhex(&id).ok_or((StatusCode::BAD_REQUEST, "id 须为 32 位 hex".into()))?;
    let _g = app.writes.lock().await;
    app.plane.remove(&bytes).map_err(err500)?;
    app.plane.persist().map_err(err500)?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn shard_stats(
    State(app): State<App>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let counts = app.plane.stats().map_err(err500)?;
    let total: u64 = counts.iter().sum();
    Ok(Json(serde_json::json!({ "counts": counts, "total": total })))
}

fn err500(e: partisync_hub::HubError) -> (StatusCode, String) {
    eprintln!("hub-demo 错误：{e}");
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

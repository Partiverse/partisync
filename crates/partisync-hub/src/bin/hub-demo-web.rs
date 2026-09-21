//! WP02/WP03 交互演示面（web）：raft 门面 + 空间注册表 + 角色 ACL。
//!
//! 展示面（浏览器单页，vanilla JS）：
//! - **数据面（raft）**：put/get/list/rename/remove 经 [`HubService"]
//!   （写 = commit+apply 后应答，读 = ReadIndex 线性一致）；分区表实时
//!   （m-meta 重载，apply 侧分裂可见）；
//! - **空间注册表**：create_space（D2 盐直显）、成员角色管理、
//!   角色矩阵 playground（check 的 OK/Forbidden 实时呈现）。
//!
//! 启动：`cargo run -p partisync-hub --bin hub-demo-web -- --addr 127.0.0.1:8091`
//! （`--root` 换库目录；Ctrl-C 退出）。

// ReplicaError 内含 openraft API 错误（体积上游决定）——豁免口径同库内模块
#![allow(clippy::result_large_err)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use partisync_hub::registry::{Action, DeviceId, RegistryError, Role};
use partisync_hub::service::{HubService, HubServiceConfig};

#[derive(Clone)]
struct App {
    svc: Arc<HubService>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = PathBuf::from("target/hub-demo-web");
    let mut addr: std::net::SocketAddr = "127.0.0.1:8091".parse()?;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = PathBuf::from(args.next().expect("--root 需要值")),
            "--addr" => addr = args.next().expect("--addr 需要值").parse()?,
            other => eprintln!("未知参数 {other}（支持 --root/--addr）"),
        }
    }
    // HubService 自带独立 runtime（raft core + 双组），axum 在外层 tokio 上
    let svc = tokio::task::spawn_blocking(move || {
        HubService::open_with_config(HubServiceConfig {
            root,
            split_threshold: 50, // 演示阈值：小目录也能看到 apply 侧分裂
            election_timeout_ms: (300, 600),
            heartbeat_interval_ms: 50,
        })
    })
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    println!("hub-demo-web：双组 raft 已就绪（pid=0 注册表 + pid=1 数据）");

    let app = App { svc: Arc::new(svc) };
    let router = Router::new()
        .route("/", get(index))
        .route("/api/status", get(status))
        .route("/api/partitions", get(partitions))
        .route("/api/entries", get(list_entries).post(create_entry))
        .route("/api/entries/{id}", get(get_entry).delete(remove_entry))
        .route("/api/rename", post(rename_entry))
        .route("/api/spaces/{id}", get(get_space).post(create_space))
        .route("/api/spaces/{id}/members", post(set_member))
        .route("/api/check", post(check))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("演示面：http://{addr}  （Ctrl-C 退出）");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("hub-demo-web.html"))
}

fn hex16(s: &str) -> Option<[u8; 16]> {
    let b = s.as_bytes();
    if b.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, chunk) in b.chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16)?;
        let lo = (chunk[1] as char).to_digit(16)?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Some(out)
}

fn to_hex(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

fn err500(e: impl std::fmt::Display) -> StatusCode {
    eprintln!("demo error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn status(State(app): State<App>) -> Json<serde_json::Value> {
    let leader = app.svc.registry_leader_hint();
    Json(serde_json::json!({
        "groups": [
            {"pid": 0, "role": "空间注册表", "leader_hint": leader},
            {"pid": 1, "role": "数据面", "leader_hint": leader},
        ],
        "semantics": "写=raft commit+apply 后应答；读=ReadIndex 线性一致",
    }))
}

async fn partitions(State(app): State<App>) -> Json<serde_json::Value> {
    let parts = app.svc.hub_partition_info();
    Json(serde_json::json!(parts
        .iter()
        .map(|(pid, start, splitting, count)| serde_json::json!({
            "pid": pid, "start_hex": start, "splitting": splitting, "count": count,
        }))
        .collect::<Vec<_>>()))
}

#[derive(serde::Deserialize)]
struct CreateReq {
    parent_hex: String,
    name: String,
    kind: String,
}

async fn create_entry(
    State(app): State<App>,
    Json(req): Json<CreateReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(parent) = hex16(&req.parent_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let mut entry_id = [0u8; 16];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    entry_id[..8].copy_from_slice(&(now as u64).to_be_bytes());
    entry_id[8..12].copy_from_slice(&now.to_be_bytes()[8..12]);
    entry_id[15] = rand_byte();
    let row = partisync_hub::EntryRow {
        entry_id,
        parent_id: Some(parent),
        kind: if req.kind == "dir" {
            partisync_hub::KIND_DIR
        } else {
            partisync_hub::KIND_FILE
        },
        name: req.name,
        content_id: None,
        size: 0,
        mtime_ns: 1,
        flags: 0,
    };
    app.svc.put_entry_async(&row).await.map_err(err500)?;
    Ok(Json(
        serde_json::json!({"id_hex": to_hex(&row.entry_id), "kind": req.kind}),
    ))
}

fn rand_byte() -> u8 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u8
}

#[derive(serde::Deserialize)]
struct ListReq {
    parent_hex: String,
}

async fn list_entries(
    State(app): State<App>,
    axum::extract::Query(req): axum::extract::Query<ListReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(parent) = hex16(&req.parent_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let page = app
        .svc
        .list_children_async(&parent, None, 200)
        .await
        .map_err(err500)?;
    Ok(Json(serde_json::json!(page
        .items
        .iter()
        .map(|it| serde_json::json!({
            "name": it.name,
            "id_hex": to_hex(&it.entry_id),
            "kind": if it.kind == partisync_hub::KIND_DIR { "dir" } else { "file" },
            "deleted": it.deleted,
        }))
        .collect::<Vec<_>>())))
}

async fn get_entry(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(id) = hex16(&id) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    match app.svc.get_entry_async(&id).await.map_err(err500)? {
        None => Ok(Json(serde_json::json!({"found": false}))),
        Some(row) => Ok(Json(serde_json::json!({
            "found": true,
            "name": row.name,
            "deleted": row.is_deleted(),
            "parent_hex": row.parent_id.map(|p| to_hex(&p)),
            "kind": if row.kind == partisync_hub::KIND_DIR { "dir" } else { "file" },
        }))),
    }
}

async fn remove_entry(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(id) = hex16(&id) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    app.svc.remove_entry_async(&id).await.map_err(err500)?;
    Ok(Json(serde_json::json!({"removed": true})))
}

#[derive(serde::Deserialize)]
struct RenameReq {
    id_hex: String,
    parent_hex: Option<String>,
    name: String,
}

async fn rename_entry(
    State(app): State<App>,
    Json(req): Json<RenameReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(id) = hex16(&req.id_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let parent = match &req.parent_hex {
        Some(h) if !h.is_empty() => Some(hex16(h).ok_or(StatusCode::BAD_REQUEST)?),
        _ => None,
    };
    app.svc
        .rename_entry_async(&id, parent, &req.name)
        .await
        .map_err(err500)?;
    Ok(Json(serde_json::json!({"renamed": true})))
}

async fn get_space(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match app.svc.registry().space_async(&id).await.map_err(err500)? {
        None => Ok(Json(serde_json::json!({"found": false}))),
        Some(row) => Ok(Json(serde_json::json!({
            "found": true,
            "root_hex": to_hex(&row.root_entry_id),
            "kdf_salt_hex": row.kdf_salt.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "members": row.members,
            "created_at_ns": row.created_at_ns,
        }))),
    }
}

#[derive(serde::Deserialize)]
struct CreateSpaceReq {
    root_hex: String,
    owner_hex: String,
}

async fn create_space(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
    Json(req): Json<CreateSpaceReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(root) = hex16(&req.root_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let Ok(owner) = DeviceId::from_hex(&req.owner_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    match app
        .svc
        .registry()
        .create_space_async(&id, root, owner)
        .await
    {
        Ok(row) => Ok(Json(serde_json::json!({
            "created": true,
            "kdf_salt_hex": row.kdf_salt.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "note": "D2：盐由 CSPRNG 生成并随 raft 命令持久化（SM apply 确定性）",
        }))),
        Err(RegistryError::Exists) => Ok(Json(
            serde_json::json!({"created": false, "reason": "Exists"}),
        )),
        Err(e) => Err(err500(e)),
    }
}

#[derive(serde::Deserialize)]
struct SetMemberReq {
    actor_hex: String,
    target_hex: String,
    role: String,
}

async fn set_member(
    State(app): State<App>,
    AxPath(id): AxPath<String>,
    Json(req): Json<SetMemberReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Ok(actor) = DeviceId::from_hex(&req.actor_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let Ok(target) = DeviceId::from_hex(&req.target_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let role = match req.role.as_str() {
        "owner" => Role::Owner,
        "editor" => Role::Editor,
        _ => Role::Viewer,
    };
    match app
        .svc
        .registry()
        .set_member_async(actor, &id, target, role)
        .await
    {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(RegistryError::Forbidden) => Ok(Json(
            serde_json::json!({"ok": false, "reason": "Forbidden（须 Owner；最后一名 Owner 防锁定）"}),
        )),
        Err(e) => Err(err500(e)),
    }
}

#[derive(serde::Deserialize)]
struct CheckReq {
    space_id: String,
    device_hex: String,
    action: String,
}

async fn check(
    State(app): State<App>,
    Json(req): Json<CheckReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Ok(device) = DeviceId::from_hex(&req.device_hex) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    let action = match req.action.as_str() {
        "read" => Action::Read,
        "write" => Action::Write,
        _ => Action::Admin,
    };
    match app
        .svc
        .registry()
        .check_async(&req.space_id, device, action)
        .await
    {
        Ok(()) => Ok(Json(serde_json::json!({"allowed": true}))),
        Err(RegistryError::Forbidden) => Ok(Json(
            serde_json::json!({"allowed": false, "reason": "Forbidden"}),
        )),
        Err(RegistryError::Missing) => Ok(Json(
            serde_json::json!({"allowed": false, "reason": "空间不存在"}),
        )),
        Err(e) => Err(err500(e)),
    }
}

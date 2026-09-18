//! 演示面路由（SPEC M0-WP02 契约 §4）：五个 JSON 端点 + 单页界面。
//!
//! 语义诚实（调研方案 §5.13）：绑定 127.0.0.1，无鉴权——公网暴露前必须加鉴权（M1 SPEC）。

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use partisync_graph::store::{DupGroup, EntryRow, Stats, Store};

/// 应用状态：仓储句柄 + 库路径（页面徽标展示）。
#[derive(Clone)]
pub struct App {
    store: Store,
    db: String,
}

/// 组装路由。
pub fn router(store: Store, db: String) -> Router {
    let app = App { store, db };
    Router::new()
        .route("/", get(index_html))
        .route("/api/stats", get(api_stats))
        .route("/api/list", get(api_list))
        .route("/api/search", get(api_search))
        .route("/api/duplicates", get(api_duplicates))
        .route("/api/breadcrumb", get(api_breadcrumb))
        .with_state(app)
}

fn err500(e: partisync_core::error::PartisyError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

async fn index_html(State(app): State<App>) -> Html<&'static str> {
    let _ = &app.db;
    Html(include_str!("ui.html"))
}

async fn api_stats(State(app): State<App>) -> Result<Json<Stats>, (StatusCode, String)> {
    app.store.stats().await.map(Json).map_err(err500)
}

async fn api_list(
    State(app): State<App>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<EntryRow>>, (StatusCode, String)> {
    let path = q.get("path").map_or("/", String::as_str);
    app.store.children(path).await.map(Json).map_err(err500)
}

async fn api_breadcrumb(
    State(app): State<App>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<EntryRow>>, (StatusCode, String)> {
    let path = q.get("path").map_or("/", String::as_str);
    app.store.ancestors_of(path).await.map(Json).map_err(err500)
}

async fn api_search(
    State(app): State<App>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<EntryRow>>, (StatusCode, String)> {
    let query = q.get("q").map_or("", String::as_str);
    app.store.search(query, 200).await.map(Json).map_err(err500)
}

async fn api_duplicates(
    State(app): State<App>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<DupGroup>>, (StatusCode, String)> {
    let limit = q.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50);
    app.store.duplicates(limit).await.map(Json).map_err(err500)
}

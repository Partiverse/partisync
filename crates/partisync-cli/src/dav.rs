//! serve webdav——最小 WebDAV 面（SPEC M1-WP00/WP05 服务面第二协议）。
//!
//! 语义声明（调研方案 §5.13）：与 serve s3 共用 `--data` 数据根（bucket/一级目录）；
//! 支持目录（collection）与文件的完整读写；无鉴权（仅本机）。
//! 已知缺口（调研方案 §3.2 备案）：Lock 未实现（返回 200 空锁信息）、
//! 目录 mtime 不可靠（fs 语义）、ETag = MD5（与 s3api 口径一致）。

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use md5::{Digest, Md5};
use std::path::PathBuf;

use crate::s3api::{http_date, xml_escape};

#[derive(Clone)]
pub struct DavApp {
    pub data_root: PathBuf,
}

pub fn router(data_root: PathBuf) -> Router {
    let app = DavApp { data_root };
    // 根与子路径双路由（根路径无 Path 参数——PROPFIND/OPTIONS 在根上必须可用）
    Router::new()
        .route("/", any(root_handler))
        .route("/{*key}", any(dav_handler))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 1024))
        .with_state(app)
}

async fn root_handler(
    State(app): State<DavApp>,
    method: axum::http::Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    dav_dispatch(&app, method, String::new(), headers, body).await
}

// ---------- 路径 ----------

fn resolve(app: &DavApp, raw: &str) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    let p = decoded.trim_start_matches('/');
    if p.split('/')
        .any(|seg| seg == ".." || seg.is_empty() && !p.is_empty())
        && p.contains("..")
    {
        return None;
    }
    Some(app.data_root.join(p))
}

fn etag_of(data: &[u8]) -> String {
    let mut h = Md5::new();
    h.update(data);
    format!("\"{:x}\"", h.finalize())
}

// ---------- 主分发 ----------

async fn dav_handler(
    State(app): State<DavApp>,
    method: axum::http::Method,
    Path(rest): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    dav_dispatch(&app, method, rest, headers, body).await
}

async fn dav_dispatch(
    app: &DavApp,
    method: axum::http::Method,
    rest: String,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let key = rest.trim_end_matches('/').to_string();
    match method.as_str() {
        "OPTIONS" => options(),
        "PROPFIND" => propfind(app, &key, headers).await,
        "GET" | "HEAD" => get_or_head(app, &key, method == axum::http::Method::HEAD).await,
        "PUT" => put(app, &key, body).await,
        "MKCOL" => mkcol(app, &key),
        "DELETE" => delete(app, &key).await,
        "MOVE" => move_resource(app, &key, &headers),
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn options() -> Response {
    (
        StatusCode::OK,
        [
            ("DAV", "1, 2"),
            (
                "Allow",
                "OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND, MKCOL, MOVE",
            ),
            ("content-length", "0"),
        ],
        axum::body::Body::empty(),
    )
        .into_response()
}

// ---------- PROPFIND ----------

async fn propfind(app: &DavApp, key: &str, headers: HeaderMap) -> Response {
    let Some(path) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    // key=""（根）→ data_root 本身
    let path = if key.is_empty() {
        app.data_root.clone()
    } else {
        path
    };
    let depth = headers
        .get("Depth")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("1");
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return (
            StatusCode::NOT_FOUND,
            [("content-type", "application/xml")],
            r#"<?xml version="1.0" encoding="utf-8"?><D:error xmlns:D="DAV:"><D:cannot-modify-protected-property/></D:error>"#.to_string(),
        )
            .into_response();
    };
    let mut xml =
        String::from(r#"<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:">"#);
    let self_href = format!("/{}", percent_encode_path(key));
    let is_dir = meta.is_dir();
    xml.push_str(&prop_response(&self_href, is_dir, meta.len(), &path));
    if depth != "0" && is_dir {
        if let Ok(rd) = std::fs::read_dir(&path) {
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let child_meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let child_href = format!(
                    "/{}{}",
                    percent_encode_path(&format!("{key}/{name}")),
                    if child_meta.is_dir() { "/" } else { "" }
                );
                xml.push_str(&prop_response(
                    &child_href,
                    child_meta.is_dir(),
                    child_meta.len(),
                    &entry.path(),
                ));
            }
        }
    }
    xml.push_str("</D:multistatus>");
    (
        StatusCode::MULTI_STATUS,
        [(header::CONTENT_TYPE, "application/xml")],
        xml,
    )
        .into_response()
}

/// 单条 response（self 或 child）。
fn prop_response(href: &str, is_dir: bool, size: u64, path: &std::path::Path) -> String {
    let resourcetype = if is_dir {
        "<D:resourcetype><D:collection/></D:resourcetype>"
    } else {
        "<D:resourcetype/>"
    };
    let (length, etag) = if is_dir {
        (String::new(), String::new())
    } else {
        let data = std::fs::read(path).unwrap_or_default();
        (
            format!("<D:getcontentlength>{size}</D:getcontentlength>"),
            { format!("<D:getetag>{}</D:getetag>", xml_escape(&etag_of(&data))) },
        )
    };
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(http_date)
        .unwrap_or_else(|_| "Thu, 01 Jan 1970 00:00:00 GMT".into());
    format!(
        "<D:response><D:href>{}</D:href><D:propstat><D:prop>{resourcetype}\
         <D:getlastmodified>{modified}</D:getlastmodified>{length}{etag}\
         <D:supportedlock></D:supportedlock></D:prop>\
         <D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
        xml_escape(href)
    )
}

fn percent_encode_path(p: &str) -> String {
    percent_encoding::utf8_percent_encode(p, percent_encoding::NON_ALPHANUMERIC).to_string()
}

// ---------- 其余方法 ----------

async fn get_or_head(app: &DavApp, key: &str, head: bool) -> Response {
    if key.is_empty() {
        return StatusCode::METHOD_NOT_ALLOWED.into_response(); // 根不是文件
    }
    let Some(path) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => {
            // 目录 GET：rclone 不走此路；给 200 空体兼容部分客户端
            (
                StatusCode::OK,
                [(header::CONTENT_LENGTH, "0")],
                axum::body::Body::empty(),
            )
                .into_response()
        }
        Ok(m) => {
            let Ok(data) = std::fs::read(&path) else {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            };
            let etag = etag_of(&data);
            let body = if head {
                axum::body::Body::empty()
            } else {
                axum::body::Body::from(data)
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(header::CONTENT_LENGTH, m.len())
                .header(header::ETAG, etag)
                .header(
                    header::LAST_MODIFIED,
                    m.modified().map(http_date).unwrap_or_default(),
                )
                .body(body)
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn put(app: &DavApp, key: &str, body: Bytes) -> Response {
    if key.is_empty() {
        return (StatusCode::METHOD_NOT_ALLOWED, "根不可写文件").into_response();
    }
    let Some(path) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    if std::fs::symlink_metadata(&path)
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        return (
            StatusCode::CONFLICT,
            [("content-type", "text/plain")],
            "目标是目录".to_string(),
        )
            .into_response();
    }
    let Some(parent) = path.parent() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if std::fs::create_dir_all(parent).is_err() {
        return (StatusCode::CONFLICT, "父目录缺失").into_response(); // WebDAV 语义：409
    }
    match std::fs::write(&path, &body) {
        Ok(()) => (
            StatusCode::CREATED,
            [(header::ETAG, etag_of(&body))],
            String::new(),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn mkcol(app: &DavApp, key: &str) -> Response {
    if key.is_empty() {
        return (StatusCode::METHOD_NOT_ALLOWED, "根已存在").into_response();
    }
    let Some(path) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    // 父不存在 → 409（WebDAV 语义）
    if path.parent().map(|p| !p.exists()).unwrap_or(true) {
        return StatusCode::CONFLICT.into_response();
    }
    match std::fs::create_dir(&path) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            StatusCode::METHOD_NOT_ALLOWED.into_response() // 405 = 已存在（RFC 4918）
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete(app: &DavApp, key: &str) -> Response {
    if key.is_empty() {
        return StatusCode::FORBIDDEN.into_response(); // 根不可删
    }
    let Some(path) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    let result = if std::fs::symlink_metadata(&path)
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        std::fs::remove_dir_all(&path)
    } else {
        std::fs::remove_file(&path)
    };
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn move_resource(app: &DavApp, key: &str, headers: &HeaderMap) -> Response {
    let Some(dest_raw) = headers.get("Destination").and_then(|v| v.to_str().ok()) else {
        return (StatusCode::BAD_REQUEST, "缺 Destination").into_response();
    };
    // Destination 可能是绝对 URL 或绝对路径：取首个 '/' 起的 path
    let dest_path: &str = match dest_raw.split_once("://") {
        Some((_, rest)) => match rest.find('/') {
            Some(i) => &rest[i..],
            None => rest,
        },
        None => dest_raw,
    };
    let Some(dest) = resolve(app, dest_path.trim_start_matches('/')) else {
        return (StatusCode::BAD_REQUEST, "invalid destination").into_response();
    };
    let Some(src) = resolve(app, key) else {
        return (StatusCode::BAD_REQUEST, "invalid source").into_response();
    };
    if src.is_dir() || dest.is_dir() {
        return (StatusCode::METHOD_NOT_ALLOWED, "目录 MOVE 未支持").into_response();
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&src, &dest) {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

//! serve s3——S3 REST 子集（SPEC M1-WP02，path-style）。
//!
//! 语义声明（调研方案 §5.13 诚实原则）：
//! - 后端为本地对象目录（`--data`，bucket = 一级目录），非图谱投影（图谱-backed S3 归后续 SPEC）；
//! - **无签名验证**（SigV4 头接受任意值）——仅限本机/可信网，鉴权归 M1 后半；
//! - 支持：PUT/GET/HEAD/DELETE 对象、ListObjectsV2（prefix/delimiter/分页）、
//!   MPU 四端点（部件落盘、Complete 拼接）；
//! - ETag = MD5（rclone S3 口径，调研方案 §3.1）。

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::time::UNIX_EPOCH;

/// 服务状态：数据根（bucket = 一级目录）。
#[derive(Clone)]
pub struct S3App {
    pub data_root: PathBuf,
}

pub fn router(data_root: PathBuf) -> Router {
    let app = S3App { data_root };
    Router::new()
        .route("/", get(list_buckets))
        .route(
            "/{bucket}",
            get(list_objects_v2).put(create_bucket).head(head_bucket),
        )
        .route(
            "/{bucket}/",
            get(list_objects_v2).put(create_bucket).head(head_bucket),
        )
        // S3 的对象操作含 POST（Create/CompleteMultipartUpload），统一 any 内部分发
        .route("/{bucket}/{*key}", any(object_handler))
        // S3 对象与 MPU part 可达 GB 级：解除 axum 默认 2MB body 限制（SPEC M1-WP02）
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
        .with_state(app)
}

// ---------- 路径安全 ----------

fn safe_key(key: &str) -> Option<PathBuf> {
    if key.split('/').any(|seg| seg == "..") {
        return None;
    }
    let rel = FsPath::new(key);
    if rel.is_absolute() {
        return None;
    }
    Some(rel.to_path_buf())
}

fn object_path(state: &S3App, bucket: &str, key: &str) -> Option<PathBuf> {
    if bucket.is_empty() || bucket.contains('/') {
        return None;
    }
    safe_key(key).map(|k| state.data_root.join(bucket).join(k))
}

fn etag_of(data: &[u8]) -> String {
    let mut h = Md5::new();
    h.update(data);
    format!("\"{:x}\"", h.finalize())
}

const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"]; // 1970-01-01=Thu
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// RFC1123 HTTP date（GET/HEAD 响应头口径——Go 的 time.Parse(ANSIC) 期望此格式，
/// ISO8601 会让 rclone 解析失败——实测踩坑）。
pub(crate) fn http_date(system: std::time::SystemTime) -> String {
    let dur = system.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let days = (secs / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    let tod = secs % 86_400;
    format!(
        "{}, {:02} {} {y:04} {:02}:{:02}:{:02} GMT",
        WEEKDAYS[(days % 7).rem_euclid(7) as usize],
        d,
        MONTHS[(m - 1) as usize],
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn rfc3339_ms(system: std::time::SystemTime) -> String {
    let dur = system.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let ms = dur.subsec_millis();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let tod = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Howard Hinnant 的 days→civil 算法（无依赖 UTC 日历）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn last_modified(path: &FsPath) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(rfc3339_ms)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".into())
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_response(code: StatusCode, body: String) -> Response {
    (code, [(header::CONTENT_TYPE, "application/xml")], body).into_response()
}

// ---------- handlers ----------

async fn list_buckets(State(app): State<S3App>) -> Response {
    let mut buckets = String::new();
    if let Ok(rd) = std::fs::read_dir(&app.data_root) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && !e.file_name().to_string_lossy().starts_with('.')
            {
                buckets.push_str(&format!(
                    "<Bucket><Name>{}</Name></Bucket>",
                    xml_escape(&e.file_name().to_string_lossy())
                ));
            }
        }
    }
    xml_response(
        StatusCode::OK,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Owner><ID>partisync</ID><DisplayName>partisync</DisplayName></Owner><Buckets>{buckets}</Buckets></ListAllMyBucketsResult>"#
        ),
    )
}

/// CreateBucket（rclone 的 Mkdir 语义：PUT /{bucket}——实测 rclone 上传前必发）。
async fn create_bucket(State(app): State<S3App>, Path(bucket): Path<String>) -> Response {
    if bucket.is_empty() || bucket.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid bucket").into_response();
    }
    match std::fs::create_dir_all(app.data_root.join(&bucket)) {
        Ok(()) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/xml")],
            String::new(),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// HeadBucket（存在性检查）。
async fn head_bucket(State(app): State<S3App>, Path(bucket): Path<String>) -> Response {
    if app.data_root.join(&bucket).is_dir() {
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn object_handler(
    State(app): State<S3App>,
    method: axum::http::Method,
    Path((bucket, key)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match method {
        axum::http::Method::PUT => put_object(&app, &bucket, &key, &q, &headers, body).await,
        axum::http::Method::GET => get_object(&app, &bucket, &key).await,
        axum::http::Method::HEAD => head_object(&app, &bucket, &key).await,
        axum::http::Method::DELETE => delete_object(&app, &bucket, &key, &q).await,
        axum::http::Method::POST => {
            // CreateMultipartUpload（?uploads）或 CompleteMultipartUpload（?uploadId）
            if q.contains_key("uploads") {
                create_mpu(&app, &bucket, &key)
            } else if let Some(id) = q.get("uploadId") {
                complete_mpu(&app, &bucket, &key, id)
            } else {
                (StatusCode::METHOD_NOT_ALLOWED, "unsupported POST").into_response()
            }
        }
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

async fn put_object(
    app: &S3App,
    bucket: &str,
    key: &str,
    q: &HashMap<String, String>,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    let Some(path) = object_path(app, bucket, key) else {
        return (StatusCode::BAD_REQUEST, "invalid key").into_response();
    };
    // UploadPart = PUT + uploadId + partNumber
    if let (Some(upload_id), Some(part)) = (q.get("uploadId"), q.get("partNumber")) {
        let parts_dir = app.data_root.join(".mpu").join(upload_id);
        if !parts_dir.is_dir() {
            return (StatusCode::NOT_FOUND, "no such upload").into_response();
        }
        let part_file = parts_dir.join(format!("part-{part}"));
        if std::fs::write(&part_file, &body).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "part write failed").into_response();
        }
        return (
            StatusCode::OK,
            [(header::ETAG, etag_of(&body))],
            String::new(),
        )
            .into_response();
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, &body) {
        Ok(()) => {
            // 用户元数据往返：rclone 的 sync 语义依赖 x-amz-meta-mtime
            // （边车树 .meta/，不进对象 LIST；实测 NOTICE 根因）
            if let Some(mtime) = headers
                .get("x-amz-meta-mtime")
                .and_then(|v| v.to_str().ok())
            {
                let meta_path = app
                    .data_root
                    .join(".meta")
                    .join(bucket)
                    .join(format!("{key}.mtime"));
                if let Some(p) = meta_path.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                let _ = std::fs::write(meta_path, mtime);
            }
            (
                StatusCode::OK,
                [(header::ETAG, etag_of(&body))],
                String::new(),
            )
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_object(app: &S3App, bucket: &str, key: &str) -> Response {
    let Some(path) = object_path(app, bucket, key) else {
        return (StatusCode::BAD_REQUEST, "invalid key").into_response();
    };
    match std::fs::read(&path) {
        Ok(data) => {
            let mut resp = (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/octet-stream"),
                    (header::ETAG, etag_of(&data).as_str()),
                    (
                        header::LAST_MODIFIED,
                        std::fs::metadata(&path)
                            .and_then(|m| m.modified())
                            .map(http_date)
                            .unwrap_or_else(|_| "Thu, 01 Jan 1970 00:00:00 GMT".into())
                            .as_str(),
                    ),
                ],
                data,
            )
                .into_response();
            if let Some(mtime) = read_meta(app, bucket, key) {
                resp.headers_mut()
                    .insert("x-amz-meta-mtime", mtime.parse().expect("mtime 头非法"));
            }
            resp
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => no_such_key(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// 读用户元数据边车。
fn read_meta(app: &S3App, bucket: &str, key: &str) -> Option<String> {
    std::fs::read_to_string(
        app.data_root
            .join(".meta")
            .join(bucket)
            .join(format!("{key}.mtime")),
    )
    .ok()
}

fn no_such_key() -> Response {
    xml_response(
        StatusCode::NOT_FOUND,
        r#"<?xml version="1.0" encoding="UTF-8"?><Error><Code>NoSuchKey</Code></Error>"#.into(),
    )
}

/// CreateMultipartUpload：S3 语义是 POST /{bucket}/{key}?uploads —— 由 bucket 后
/// 的对象路由 POST 承接（在 router 上以 post(get_object) 兜底，见下）。
fn create_mpu(app: &S3App, bucket: &str, key: &str) -> Response {
    let id = format!("mpu-{}", partisync_core::Ulid::now());
    let dir = app.data_root.join(".mpu").join(&id);
    if std::fs::create_dir_all(&dir).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "mpu init failed").into_response();
    }
    // 记录目标路径：Complete 时按 key 重建（避免存绝对路径）
    let meta = format!("{bucket}\n{key}");
    if std::fs::write(dir.join("target"), meta).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "mpu meta failed").into_response();
    }
    xml_response(
        StatusCode::OK,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><InitiateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Bucket>{}</Bucket><Key>{}</Key><UploadId>{id}</UploadId></InitiateMultipartUploadResult>"#,
            xml_escape(bucket),
            xml_escape(key)
        ),
    )
}

fn complete_mpu(app: &S3App, _bucket: &str, _key: &str, upload_id: &str) -> Response {
    let parts_dir = app.data_root.join(".mpu").join(upload_id);
    let Ok(meta) = std::fs::read_to_string(parts_dir.join("target")) else {
        return (StatusCode::NOT_FOUND, "no such upload").into_response();
    };
    let mut lines = meta.splitn(2, '\n');
    let Some(bucket) = lines.next() else {
        return (StatusCode::NOT_FOUND, "no target").into_response();
    };
    let Some(key) = lines.next() else {
        return (StatusCode::NOT_FOUND, "no target").into_response();
    };
    let Some(dest) = object_path(app, bucket, key) else {
        return (StatusCode::BAD_REQUEST, "invalid target").into_response();
    };
    // 按 part 编号顺序拼接
    let mut parts: Vec<_> = std::fs::read_dir(&parts_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("part-"))
                .map(|e| e.path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    parts.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().trim_start_matches("part-").to_string())
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    let mut buf = Vec::new();
    for p in parts {
        if std::fs::read(p).map(|d| buf.extend(d)).is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "part read failed").into_response();
        }
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&dest, &buf) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&parts_dir);
            xml_response(
                StatusCode::OK,
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Location>partisync</Location><Bucket>{}</Bucket><Key>{}</Key><ETag>{}</ETag></CompleteMultipartUploadResult>"#,
                    xml_escape(bucket),
                    xml_escape(key),
                    etag_of(&buf)
                ),
            )
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn head_object(app: &S3App, bucket: &str, key: &str) -> Response {
    let Some(path) = object_path(app, bucket, key) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    // S3 语义：目录不是对象——HEAD 必须对目录 404（rclone 依赖此判定，
    // 否则把目录当文件、状态机错乱挂死——实测踩坑）
    match std::fs::metadata(&path) {
        Ok(m) if m.is_dir() => StatusCode::NOT_FOUND.into_response(),
        Ok(m) => {
            let mut resp = (
                StatusCode::OK,
                [
                    (header::CONTENT_LENGTH, m.len().to_string()),
                    (header::ETAG, {
                        let data = std::fs::read(&path).unwrap_or_default();
                        etag_of(&data)
                    }),
                ],
                axum::body::Body::empty(),
            )
                .into_response();
            if let Some(mtime) = read_meta(app, bucket, key) {
                resp.headers_mut()
                    .insert("x-amz-meta-mtime", mtime.parse().expect("mtime 头非法"));
            }
            resp
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn delete_object(
    app: &S3App,
    bucket: &str,
    key: &str,
    q: &HashMap<String, String>,
) -> Response {
    let Some(path) = object_path(app, bucket, key) else {
        return (StatusCode::BAD_REQUEST, "invalid key").into_response();
    };
    // AbortMultipartUpload：DELETE + uploadId
    if let Some(id) = q.get("uploadId") {
        let _ = std::fs::remove_dir_all(app.data_root.join(".mpu").join(id));
        return StatusCode::NO_CONTENT.into_response();
    }
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(
        app.data_root
            .join(".meta")
            .join(bucket)
            .join(format!("{key}.mtime")),
    );
    StatusCode::NO_CONTENT.into_response() // 幂等删除
}

async fn list_objects_v2(
    State(app): State<S3App>,
    Path(bucket): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let prefix = q.get("prefix").cloned().unwrap_or_default();
    let delimiter = q.get("delimiter").cloned().unwrap_or_default();
    let max_keys = q
        .get("max-keys")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1000)
        .min(1000);
    let after = q
        .get("continuation-token")
        .or_else(|| q.get("start-after"))
        .cloned()
        .unwrap_or_default();
    let base = app.data_root.join(&bucket);
    // 遍历根跟随 prefix（磁盘目录 = base/prefix 截尾）——否则首层子项 rel 拼接错位
    // （实测踩坑：prefix=src/ 时出现假 CommonPrefix "src/src/"）
    let scan_root = if prefix.is_empty() {
        base.clone()
    } else {
        base.join(prefix.trim_end_matches('/'))
    };
    let mut walker: Vec<(String, u64, String)> = Vec::new();
    let mut common: std::collections::BTreeSet<String> = Default::default();
    collect_dir(&scan_root, &prefix, &delimiter, &mut walker, &mut common);
    walker.sort_by(|a, b| a.0.cmp(&b.0));
    let keys: Vec<&(String, u64, String)> = walker
        .iter()
        .filter(|(k, _, _)| k.as_str() > after.as_str())
        .collect();
    let truncated = keys.len() > max_keys;
    let page: Vec<&(String, u64, String)> = keys.into_iter().take(max_keys).collect();

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );
    xml.push_str(&format!("<Name>{}</Name>", xml_escape(&bucket)));
    xml.push_str(&format!("<Prefix>{}</Prefix>", xml_escape(&prefix)));
    xml.push_str(&format!(
        "<KeyCount>{}</KeyCount>",
        page.len() + common.len()
    ));
    xml.push_str(&format!("<MaxKeys>{max_keys}</MaxKeys>"));
    xml.push_str(&format!("<IsTruncated>{}</IsTruncated>", truncated));
    if truncated {
        if let Some(tok) = page.last() {
            xml.push_str(&format!(
                "<NextContinuationToken>{}</NextContinuationToken>",
                xml_escape(&tok.0)
            ));
        }
    }
    for (k, size, etag) in &page {
        xml.push_str(&format!(
            "<Contents><Key>{}</Key><LastModified>{}</LastModified><ETag>&quot;{etag}&quot;</ETag><Size>{size}</Size><StorageClass>STANDARD</StorageClass></Contents>",
            xml_escape(k),
            last_modified(&scan_root.join(k.trim_start_matches(prefix.as_str())))
        ));
    }
    for cp in &common {
        xml.push_str(&format!(
            "<CommonPrefixes><Prefix>{}</Prefix></CommonPrefixes>",
            xml_escape(cp)
        ));
    }
    xml.push_str("</ListBucketResult>");
    xml_response(StatusCode::OK, xml)
}

/// 递归收集（prefix 过滤 + delimiter 折叠 CommonPrefixes）。
fn collect_dir(
    dir: &FsPath,
    prefix: &str,
    delimiter: &str,
    out: &mut Vec<(String, u64, String)>,
    common: &mut std::collections::BTreeSet<String>,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue; // .meta/.mpu 运行时树不进对象空间
        }
        let rel = format!("{prefix}{name}");
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            let dir_prefix = format!("{rel}/");
            if !delimiter.is_empty() {
                common.insert(dir_prefix);
                continue;
            }
            collect_dir(&entry.path(), &dir_prefix, delimiter, out, common);
        } else {
            if !rel.starts_with(prefix) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let etag = std::fs::read(entry.path())
                .map(|d| {
                    let mut h = Md5::new();
                    h.update(&d);
                    format!("{:x}", h.finalize())
                })
                .unwrap_or_default();
            out.push((rel, size, etag));
        }
    }
}

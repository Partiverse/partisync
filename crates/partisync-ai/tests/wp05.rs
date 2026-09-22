//! M4-WP05 验收：C2PA 摄取校验与保留（SPEC M4-WP05 验收标准逐条）。
//!
//! 签名样本 = 本地 image crate 生成 JPEG/PNG + ES256 自签证书
//! （tests/fixtures/es256_test.{cert,key}，test-only，经
//! `create_signer::from_keys` 签发——离线确定性，无 openssl CLI 依赖）。

use std::io::Cursor;
use std::sync::Arc;

use partisync_ai::{
    stage_ids, C2paStage, InMemoryBlobSink, InMemoryContentLoader, MimeKind, Pipeline,
    SidecarStage, StageError, StageInput,
};
use partisync_graph::store::Store;

// ─── 样本生成 ───────────────────────────────────────────────────────

fn jpeg_bytes(seed: u8) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 48, |x, y| {
        image::Rgb([x as u8 ^ seed, y as u8 ^ seed, seed])
    }));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
    buf.into_inner()
}

fn png_bytes(seed: u8) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 48, |x, y| {
        image::Rgb([x as u8 ^ seed, y as u8 ^ seed, seed])
    }));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

/// 用 ES256 测试证书给容器字节签发 C2PA manifest，返回带清单的容器。
fn signed(bytes: Vec<u8>, format: &str) -> Vec<u8> {
    let cert = include_bytes!("fixtures/es256_test.cert").as_slice();
    let key = include_bytes!("fixtures/es256_test.key").as_slice();
    let signer = c2pa::create_signer::from_keys(
        cert,
        key,
        c2pa::crypto::raw_signature::SigningAlg::Es256,
        None,
    )
    .expect("from_keys");
    let context = Arc::new(c2pa::Context::new());
    let mut builder = c2pa::Builder::from_shared_context(&context)
        .with_definition(
            // 首个 c2pa.actions 须含 created/opened（C2PA 规范校验：
            // assertion.action.malformed），否则读回即 Invalid。
            r#"{"title":"wp05-test","format":"image/jpeg",
                "claim_generator_info":[{"name":"partisync-wp05-test","version":"0.1.0"}],
                "assertions":[{"label":"c2pa.actions","data":{"actions":[
                    {"action":"c2pa.created",
                     "digitalSourceType":"http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture"}]}}]}"#,
        )
        .expect("with_definition");
    let mut source = Cursor::new(bytes);
    let mut dest = Cursor::new(Vec::new());
    builder
        .sign(signer.as_ref(), format, &mut source, &mut dest)
        .expect("sign");
    dest.into_inner()
}

fn stage_input(mime: MimeKind, data: Vec<u8>) -> StageInput {
    StageInput {
        content_id: "t".into(),
        mime,
        data,
    }
}

// ─── stage 契约 ─────────────────────────────────────────────────────

/// 验收「applicable」：image/video/audio 适用，text/other 不适用。
#[test]
fn applicable_matrix() {
    let s = C2paStage::new();
    assert_eq!(s.stage(), stage_ids::C2PA);
    assert!(s.applicable(&MimeKind::Image));
    assert!(s.applicable(&MimeKind::Video));
    assert!(s.applicable(&MimeKind::Audio));
    assert!(!s.applicable(&MimeKind::Text));
    assert!(!s.applicable(&MimeKind::Other));
}

/// 验收「无 manifest」：done + state=absent（完成结论，非失败非降级）。
#[test]
fn absent_on_unsigned() {
    let out = C2paStage::new()
        .run(&stage_input(MimeKind::Image, png_bytes(1)))
        .expect("plain png 校验完成");
    assert_eq!(out.detail.as_deref(), Some("state=absent"));
    assert!(out.blob.is_none(), "无清单不产 blob（列保持 NULL）");
}

/// 验收「校验+保留」：签名样本 → state=Valid + blob = manifest store
/// report JSON（可解析、含 active manifest）。
#[test]
fn validates_and_preserves_signed() {
    let data = signed(jpeg_bytes(2), "image/jpeg");
    let out = C2paStage::new()
        .run(&stage_input(MimeKind::Image, data))
        .expect("signed jpeg 解析");
    let detail = out.detail.expect("detail");
    assert!(
        detail.starts_with("state=Valid"),
        "期望 Valid，实得 {detail}"
    );
    let json: serde_json::Value =
        serde_json::from_slice(out.blob.as_deref().expect("blob")).expect("manifest JSON");
    assert!(
        json.get("manifests").is_some(),
        "manifest store report 形态"
    );
}

/// 验收「无效清单同样保留」：篡改扫描数据（容器结构不变、数据哈希失配）
/// → state=Invalid 且清单 blob 仍产出。翻转位锚定文件尾（JPEG 布局：
/// JUMBF 盒在前、扫描数据贴 EOI——清单盒内字节属 dataHash 排除区，
/// 尾部扫描字节才构成哈希失配；样本内容固定故布局确定）。
#[test]
fn tampered_still_preserved_as_invalid() {
    let mut data = signed(jpeg_bytes(3), "image/jpeg");
    let mid = data.len() - 100;
    data[mid] ^= 0xFF;
    let out = C2paStage::new()
        .run(&stage_input(MimeKind::Image, data))
        .expect("篡改样本仍应解析出清单");
    let detail = out.detail.expect("detail");
    assert!(
        detail.starts_with("state=Invalid"),
        "期望 Invalid，实得 {detail}"
    );
    assert!(out.blob.is_some(), "无效清单同样保留");
}

/// 验收「容器不支持 → skipped」：非容器字节（嗅探不出已知容器 →
/// UnsupportedType，判定表归降级而非失败）。
#[test]
fn unsupported_container_skipped() {
    let garbage = [0xDEu8, 0xAD, 0xBE, 0xEF].repeat(64);
    let r = C2paStage::new().run(&stage_input(MimeKind::Image, garbage));
    match r {
        Err(StageError::Skipped(reason)) => {
            assert_eq!(reason, "format-unsupported-by-c2pa");
        }
        other => panic!("期望 Skipped，实得 {other:?}"),
    }
}

// ─── 管线级（DB 列回写 / 降级 / 幂等） ─────────────────────────────

async fn mem_store() -> Store {
    Store::open_in_memory().await.unwrap()
}

async fn seed_content(store: &Store, id: &str, mime: &str) {
    sqlx::query("INSERT OR IGNORE INTO content (id, size, mime, kind) VALUES (?, 1, ?, 'file')")
        .bind(id)
        .bind(mime)
        .execute(store.pool_ref())
        .await
        .unwrap();
}

async fn c2pa_column(store: &Store, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT c2pa FROM content WHERE id = ?")
        .bind(id)
        .fetch_one(store.pool_ref())
        .await
        .unwrap()
}

fn c2pa_only_pipeline() -> Pipeline {
    Pipeline::new(
        Arc::new(InMemoryBlobSink::new()),
        vec![Arc::new(C2paStage::new())],
    )
}

/// 验收「列回写」：签名样本跑管线 → content.c2pa 列写入可解析 JSON。
#[tokio::test]
async fn pipeline_writes_content_column() {
    let store = mem_store().await;
    seed_content(&store, "c-signed", "image/jpeg").await;
    let loader =
        InMemoryContentLoader::new([("c-signed".into(), signed(jpeg_bytes(4), "image/jpeg"))]);
    let s = c2pa_only_pipeline()
        .run_content(&store, &loader, "c-signed")
        .await
        .unwrap();
    assert_eq!((s.done, s.skipped, s.failed), (1, 0, 0));
    let col = c2pa_column(&store, "c-signed").await.expect("列已写");
    let json: serde_json::Value = serde_json::from_str(&col).expect("列内 JSON 可解析");
    assert!(json.get("manifests").is_some());
}

/// 验收「无清单 → 列 NULL」+ 幂等重跑（终态不改写）。
#[tokio::test]
async fn pipeline_absent_and_idempotent() {
    let store = mem_store().await;
    seed_content(&store, "c-plain", "image/png").await;
    let loader = InMemoryContentLoader::new([("c-plain".into(), png_bytes(5))]);
    let pipeline = c2pa_only_pipeline();
    let s = pipeline
        .run_content(&store, &loader, "c-plain")
        .await
        .unwrap();
    assert_eq!((s.done, s.skipped, s.failed), (1, 0, 0));
    assert!(
        c2pa_column(&store, "c-plain").await.is_none(),
        "列保持 NULL"
    );

    // 重跑：done 终态不重算
    let s2 = pipeline
        .run_content(&store, &loader, "c-plain")
        .await
        .unwrap();
    assert_eq!((s2.done, s2.skipped, s2.failed), (0, 0, 0), "重跑 no-op");
    assert!(c2pa_column(&store, "c-plain").await.is_none());
}

/// 验收「不适用 mime」：text/plain → skipped(not-applicable)，列 NULL。
#[tokio::test]
async fn pipeline_not_applicable() {
    let store = mem_store().await;
    seed_content(&store, "c-text", "text/plain").await;
    let loader = InMemoryContentLoader::new([("c-text".into(), b"hello".to_vec())]);
    let s = c2pa_only_pipeline()
        .run_content(&store, &loader, "c-text")
        .await
        .unwrap();
    assert_eq!((s.done, s.skipped, s.failed), (0, 1, 0));
    let (status, detail): (i64, Option<String>) = sqlx::query_as(
        "SELECT status, detail FROM sidecar_items WHERE content_id = ? AND stage = ?",
    )
    .bind("c-text")
    .bind(stage_ids::C2PA)
    .fetch_one(store.pool_ref())
    .await
    .unwrap();
    assert_eq!(status, 3, "skipped");
    assert_eq!(detail.as_deref(), Some("not-applicable"));
    assert!(c2pa_column(&store, "c-text").await.is_none());
}

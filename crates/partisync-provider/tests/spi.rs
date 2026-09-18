//! Provider SPI 测试（SPEC M1-WP01 验收）：fs provider 全操作 + caps 判定。

use partisync_core::caps::MtimePrecision;
use partisync_core::Ulid;
use partisync_provider::config::{ProviderConfig, ProviderScheme};
use partisync_provider::Provider;

fn fs_config(root: &str) -> ProviderConfig {
    let mut params = serde_json::Map::new();
    params.insert("root".into(), serde_json::json!(root));
    ProviderConfig {
        scheme: ProviderScheme::Fs,
        params,
    }
}

#[tokio::test]
async fn fs_provider_roundtrip() {
    let dir = std::env::temp_dir().join(format!("prov-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = Provider::from_config(&fs_config(dir.to_str().unwrap())).unwrap();

    p.write_file("hello.txt", b"provider roundtrip".to_vec())
        .await
        .unwrap();
    assert!(p.exists("hello.txt").await.unwrap());
    assert_eq!(
        p.read_file("hello.txt").await.unwrap(),
        b"provider roundtrip"
    );

    // 目录结构：子目录 + 列表
    p.write_file("sub/inner.txt", b"inner".to_vec())
        .await
        .unwrap();
    let entries = p.list_children("/").await.unwrap();
    eprintln!("DEBUG list /: {:?}", entries);
    assert!(
        entries
            .iter()
            .any(|e| e.is_dir && e.path.trim_end_matches('/') == "sub"),
        "sub 目录应出现（实际 {:?}）",
        entries
    );
    assert!(entries.iter().any(|e| !e.is_dir && e.path == "hello.txt"));
    let inner = p.list_children("sub").await.unwrap();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].path, "sub/inner.txt");
    assert_eq!(inner[0].size, 5);

    p.delete("hello.txt").await.unwrap();
    assert!(!p.exists("hello.txt").await.unwrap());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn caps_are_conservative_and_scheme_specific() {
    // P9：webdav 全保守（Default）
    let wd = ProviderConfig {
        scheme: ProviderScheme::Webdav,
        params: serde_json::Map::new(),
    };
    assert_eq!(wd.caps(), partisync_core::caps::ProviderCaps::default());

    // fs：BLAKE3 可自算、mtime Nanos
    let fs_caps = fs_config("/tmp").caps();
    assert!(fs_caps.hash.blake3);
    assert_eq!(fs_caps.mtime, MtimePrecision::Nanos);

    // s3：MPU/预签名/MD5+SHA256+CRC64NVMe、Millis、无原子改名
    let mut params = serde_json::Map::new();
    params.insert("bucket".into(), serde_json::json!("b"));
    let s3 = ProviderConfig {
        scheme: ProviderScheme::S3,
        params,
    }
    .caps();
    assert!(s3.multipart && s3.presign_put && !s3.atomic_rename);
    assert!(s3.hash.md5 && s3.hash.sha256 && s3.hash.crc64nvme && !s3.hash.blake3);
    assert_eq!(s3.mtime, MtimePrecision::Millis);
}

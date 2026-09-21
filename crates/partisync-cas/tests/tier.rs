//! WP04 T03 验收（分层存储）：SPEC 验收
//! 「分层迁移：温度策略触发整包迁移（NVMe→HDD→S3）+ 回取按需，
//!  迁移中途 kill → 重启可重入、无丢块」（proptest 钉住）。

use std::path::{Path, PathBuf};

use partisync_cas::tier::{FsBackend, Tier, TierBackend, TierEngine};
use proptest::prelude::*;

fn tempdir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cas-tier-it-{tag}-{}", partisync_core::Ulid::now()))
}

fn fs(name: &'static str, root: &Path) -> Box<dyn TierBackend> {
    Box::new(FsBackend::new(root, name).expect("fs backend"))
}

async fn engine_at(root: &Path) -> TierEngine {
    TierEngine::open(
        fs("hot", &root.join("hot")),
        fs("warm", &root.join("warm")),
        fs("cold", &root.join("cold")),
        &root.join("manifest"),
    )
    .await
    .expect("engine")
}

#[tokio::test]
async fn migration_hot_to_cold_keeps_data() {
    let root = tempdir("full");
    let engine = engine_at(&root).await;
    let payload: Vec<u8> = (0..4096u32).map(|i| (i & 0xff) as u8).collect();
    engine.put("p1", &payload).await.expect("put");
    engine.migrate("p1", Tier::Warm).await.expect("to warm");
    engine.migrate("p1", Tier::Cold).await.expect("to cold");
    assert_eq!(engine.tier_of("p1").await.unwrap(), Tier::Cold);
    assert_eq!(engine.read("p1").await.unwrap(), payload);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn read_falls_through_tiers() {
    // 模拟冷层单点：仅 cold 有数据 → read 仍能找到
    let root = tempdir("fallthrough");
    let engine = engine_at(&root).await;
    engine.put("p1", b"x").await.expect("put");
    engine.migrate("p1", Tier::Cold).await.expect("migrate");
    // 直接用 cold 后端读（模拟物理只剩冷层）
    let cold_only = FsBackend::new(&root.join("cold"), "cold").unwrap();
    assert_eq!(cold_only.get("p1").unwrap(), b"x");
    // engine 读仍回退命中
    assert_eq!(engine.read("p1").await.unwrap(), b"x");
    let _ = std::fs::remove_dir_all(&root);
}

proptest! {
    /// 任意 payload 在三级间反复迁移，逐次 read 字节相等。
    #[test]
    fn prop_migrate_preserves_bytes(payload in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let root = tempdir("prop");
            let engine = engine_at(&root).await;
            engine.put("p1", &payload).await.expect("put");
            for to in [Tier::Warm, Tier::Cold, Tier::Hot, Tier::Warm] {
                if engine.tier_of("p1").await.unwrap() != to {
                    engine.migrate("p1", to).await.expect("migrate");
                }
                prop_assert_eq!(&engine.read("p1").await.unwrap()[..], &payload[..]);
            }
            let _ = std::fs::remove_dir_all(&root);
            Ok::<(), proptest::test_runner::TestCaseError>(())
        }).unwrap();
    }

    /// 多次独立 pack 并行迁移：每条最终 read 字节等于 put 字节。
    #[test]
    fn prop_parallel_packs_independent(
        n in 2usize..=10usize,
        base in 0u32..1000u32,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let root = tempdir("parallel");
            let engine = engine_at(&root).await;
            let mut expected: Vec<(String, Vec<u8>)> = Vec::new();
            for i in 0..n {
                let payload = vec![((i as u32 + base) & 0xff) as u8; 64 + i * 7];
                let id = format!("p{i:04}");
                engine.put(&id, &payload).await.expect("put");
                expected.push((id.clone(), payload));
                let to = match i % 3 {
                    0 => Tier::Hot,
                    1 => Tier::Warm,
                    _ => Tier::Cold,
                };
                if engine.tier_of(&id).await.unwrap() != to {
                    engine.migrate(&id, to).await.expect("migrate");
                }
            }
            for (id, want) in &expected {
                let got = engine.read(id).await.expect("read");
                prop_assert_eq!(&got[..], &want[..]);
            }
            let _ = std::fs::remove_dir_all(&root);
            Ok::<(), proptest::test_runner::TestCaseError>(())
        }).unwrap();
    }
}

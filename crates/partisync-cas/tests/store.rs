//! 块库集成测试（P4，SPEC M0-WP03 验收）+ checkpoint 端到端场景。

use partisync_cas::chunker::{chunk_boundaries, chunk_root, CdcConfig};
use partisync_cas::{content_hash, put_chunks, ChunkStore};

fn test_cfg() -> CdcConfig {
    CdcConfig::new(64, 256, 1024).unwrap()
}

async fn mem_store() -> ChunkStore {
    let dir = std::env::temp_dir().join(format!("cas-{}", partisync_core::Ulid::now()));
    ChunkStore::open_in_memory(&dir).await.unwrap()
}

/// P4a：root(chunks) 口径——chunk_root == content_hash(concat(hex))，往返一致。
#[tokio::test]
async fn p4_chunk_root_matches_content_of_hashes() {
    let data: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
    let cfg = test_cfg();
    let chunks = chunk_boundaries(&data, cfg);
    assert!(chunks.len() > 1);
    let hashes: Vec<String> = chunks
        .iter()
        .map(|(o, l)| content_hash(&data[*o..*o + l]))
        .collect();
    let refs: Vec<&str> = hashes.iter().map(String::as_str).collect();
    let root = chunk_root(&refs);
    assert_eq!(root, content_hash(refs.concat().as_bytes()));
}

/// P4b：同内容二次 put → refcount+1、不新增对象；get 返回原字节。
#[tokio::test]
async fn p4_put_same_content_increments_refcount() {
    let s = mem_store().await;
    let data = b"identical chunk payload";
    let h1 = s.put(data).await.unwrap();
    let st1 = s.stats().await.unwrap();
    assert_eq!(
        st1.saved_bytes, 0,
        "单引用无节省（绝对值断言：-/+ 变异的击杀点）"
    );
    let h2 = s.put(data).await.unwrap();
    assert_eq!(h1, h2);
    let st2 = s.stats().await.unwrap();
    assert_eq!(
        st2.saved_bytes,
        data.len() as i64,
        "两引用一唯一块 ⇒ 恰省一份"
    );
    assert_eq!(st2.chunks, st1.chunks, "不新增块");
    assert_eq!(st2.refs, st1.refs + 1, "refcount+1");
    assert_eq!(st2.saved_bytes, st1.saved_bytes + data.len() as i64);
    assert_eq!(s.get(&h1).await.unwrap(), data);
}

/// P4c：decr 归零 → 行与对象删除；再 put 可重建。
#[tokio::test]
async fn p4_decr_to_zero_deletes_object() {
    let dir = std::env::temp_dir().join(format!("cas-del-{}", partisync_core::Ulid::now()));
    let s = ChunkStore::open_in_memory(&dir).await.unwrap();
    let h = s.put(b"to be deleted").await.unwrap();
    let obj = dir.join("objects").join(&h[..2]).join(&h);
    assert!(obj.exists(), "对象文件存在");
    assert_eq!(s.decr(&h).await.unwrap(), 0);
    assert!(!obj.exists(), "归零后对象删除");
    let st = s.stats().await.unwrap();
    assert_eq!((st.chunks, st.refs), (0, 0));
    let err = s.get(&h).await.unwrap_err();
    assert!(
        err.to_string().contains("块不存在"),
        "删除后 get 的错误应区分 NotFound（当前: {err}）"
    );
    let h2 = s.put(b"to be deleted").await.unwrap();
    assert_eq!(h, h2, "内容寻址：重建得同哈希");
    std::fs::remove_dir_all(&dir).ok();
}

/// checkpoint 端到端（SPEC 验收）：6 版本 × 2MB 基底、尾部 5% 变异
/// ⇒ 块级 saved_bytes ≥ 总引用字节 70%。
#[tokio::test]
async fn checkpoint_versions_dedup_heavily() {
    let s = mem_store().await;
    let cfg = CdcConfig::new(1024, 4096, 16384).unwrap();
    let mut total_ref_bytes = 0i64;
    let mut base: Vec<u8> = (0..2 * 1024 * 1024u32)
        .map(|i| (i * 31 % 253) as u8)
        .collect();
    for version in 0..6u8 {
        // 尾部 5% 变异（模拟 checkpoint 的少量新增/修改）
        let tail = base.len() - base.len() / 20;
        for (i, b) in base[tail..].iter_mut().enumerate() {
            *b = b.wrapping_add(version).wrapping_add(i as u8);
        }
        let (_, _root) = put_chunks(&s, &base, cfg).await.unwrap();
        total_ref_bytes += base.len() as i64;
    }
    let st = s.stats().await.unwrap();
    assert!(
        st.saved_bytes * 100 >= total_ref_bytes * 70,
        "块级节省 {}/{} 未达 70%",
        st.saved_bytes,
        total_ref_bytes
    );
    // 块数应远小于 6 份全量块的份数（共享块被复用）
    let full_chunks = total_ref_bytes / 4096;
    assert!(
        st.chunks * 3 < full_chunks,
        "块数 {} 未表现出共享（全量约为 {} 块）",
        st.chunks,
        full_chunks
    );
}

/// put_chunks 便捷函数：分块入库 → 全部块可取回、拼接等于原文。
#[tokio::test]
async fn put_chunks_roundtrip() {
    let s = mem_store().await;
    let data: Vec<u8> = (0..30000u32).map(|i| (i * 13 % 249) as u8).collect();
    let (hashes, root) = put_chunks(&s, &data, test_cfg()).await.unwrap();
    let mut joined = Vec::with_capacity(data.len());
    for h in &hashes {
        joined.extend_from_slice(&s.get(h).await.unwrap());
    }
    assert_eq!(joined, data);
    let refs: Vec<&str> = hashes.iter().map(String::as_str).collect();
    assert_eq!(root, chunk_root(&refs));
}

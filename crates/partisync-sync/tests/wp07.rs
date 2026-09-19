//! M2-WP07 验收测试（SPEC 验收标准）：KDF 全链确定性 + encrypt round-trip +
//! 篡改拒绝 + 密钥域分离 + 空间注册 round-trip。

use blake3::Hash;
use partisync_core::Ulid;
use partisync_graph::store::Store;
use partisync_sync::crypto;

async fn node(tag: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("wp7-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume("dev-x", "dev-x", "dev-x")
        .await
        .unwrap();
    s
}

/// 计算 master_key 的"持有证明"哈希（v1 简化 = blake3 截位 hex）。
fn kek_hash_of(master: &[u8; 32]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(master);
    h.update(b"|kek-attest-v1");
    let out = h.finalize();
    hex(&out.as_bytes()[..16])
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

#[tokio::test]
async fn space_crypto_register_and_recall() {
    let s = node("reg").await;
    assert!(s.space_crypto("default").await.unwrap().is_none());
    s.register_space_crypto("default", "abc123", "xchacha20-blake3-v1")
        .await
        .unwrap();
    let row = s.space_crypto("default").await.unwrap().unwrap();
    assert_eq!(row.kek_hash, "abc123");
    assert_eq!(row.alg, "xchacha20-blake3-v1");
}

#[tokio::test]
async fn space_crypto_register_is_overwrite_on_rotation() {
    let s = node("rot").await;
    s.register_space_crypto("default", "v1", "xchacha20-blake3-v1")
        .await
        .unwrap();
    s.register_space_crypto("default", "v2", "xchacha20-blake3-v1")
        .await
        .unwrap();
    let row = s.space_crypto("default").await.unwrap().unwrap();
    assert_eq!(row.kek_hash, "v2", "轮换：kek_hash 覆盖");
}

#[tokio::test]
async fn kdf_chain_full_determinism() {
    // 同一 mnemonic+space_id 派生：master → space → content/meta ⇒ 全部稳定
    let mnemonic =
        "abandon ability able about above absent absorb abstract absurd abuse access accident";
    let space = "default";
    let master1 = crypto::argon2_master_key(mnemonic, space);
    let master2 = crypto::argon2_master_key(mnemonic, space);
    assert_eq!(master1, master2, "Argon2 master 派生确定性");
    let sk1 = crypto::derive_space_key(&master1, space);
    let sk2 = crypto::derive_space_key(&master2, space);
    assert_eq!(sk1, sk2, "space_key 派生确定性");
    let ck1 = crypto::derive_content_key(&sk1, "content-A");
    let ck2 = crypto::derive_content_key(&sk2, "content-A");
    assert_eq!(ck1, ck2, "content_key 派生确定性");
    let mk1 = crypto::derive_meta_key(&sk1);
    let mk2 = crypto::derive_meta_key(&sk2);
    assert_eq!(mk1, mk2, "meta_key 派生确定性");
}

#[tokio::test]
async fn different_space_yields_different_keys() {
    let master = [9u8; 32];
    let k1 = crypto::derive_space_key(&master, "alpha");
    let k2 = crypto::derive_space_key(&master, "beta");
    assert_ne!(k1, k2, "不同空间隔离");
    let ck_a = crypto::derive_content_key(&k1, "content-X");
    let ck_b = crypto::derive_content_key(&k2, "content-X");
    assert_ne!(ck_a, ck_b, "同名 content 在不同空间密钥下加密结果必不同");
}

#[tokio::test]
async fn encrypt_decrypt_roundtrip_with_kdf_chain() {
    let master = crypto::argon2_master_key(
        "abandon ability able about above absent absorb abstract absurd abuse access accident",
        "default",
    );
    let sk = crypto::derive_space_key(&master, "default");
    let ck = crypto::derive_content_key(&sk, "content-id-hello");
    let plaintext = b"hello e2ee world, encrypted content bytes";
    let (ct, nonce) = crypto::encrypt_content(&ck, plaintext);
    let pt = crypto::decrypt_content(&ck, &nonce, &ct).unwrap();
    assert_eq!(pt, plaintext);
}

#[tokio::test]
async fn decrypt_rejects_byte_tampering() {
    let master = crypto::argon2_master_key(
        "abandon ability able about above absent absorb abstract absurd abuse access accident",
        "default",
    );
    let ck = crypto::derive_content_key(&master, "x");
    let (mut ct, nonce) = crypto::encrypt_content(&ck, b"abcdef");
    let n = ct.len();
    ct[n - 1] ^= 0x01;
    let err = crypto::decrypt_content(&ck, &nonce, &ct).unwrap_err();
    assert!(err.to_string().contains("AEAD"));
}

#[tokio::test]
async fn decrypt_rejects_wrong_key() {
    let k1 = [1u8; 32];
    let k2 = [2u8; 32];
    let (ct, nonce) = crypto::encrypt_content(&k1, b"hello");
    assert!(crypto::decrypt_content(&k2, &nonce, &ct).is_err());
}

#[tokio::test]
async fn register_with_kek_hash_round_trip_persists() {
    // 完整链路：master → kek_hash 落库 → 再次查询能验证持有证明
    let s = node("kek").await;
    let master = crypto::argon2_master_key(
        "abandon ability able about above absent absorb abstract absurd abuse access accident",
        "default",
    );
    let kek = kek_hash_of(&master);
    s.register_space_crypto("default", &kek, "xchacha20-blake3-v1")
        .await
        .unwrap();
    let row = s.space_crypto("default").await.unwrap().unwrap();
    assert_eq!(row.kek_hash, kek);
    assert_eq!(
        row.kek_hash.len(),
        32,
        "kek_hash 应为 16 字节 hex = 32 字符"
    );
}

#[tokio::test]
async fn kdf_salt_persist_first_write_wins() {
    // SEC-AUDIT P2-3：生产空间持随机持久盐；首写固定（盐轮换=作废已加密内容）
    let s = node("salt").await;
    assert!(s.space_kdf_salt("default").await.unwrap().is_none());
    let salt = crypto::random_kdf_salt();
    s.upsert_space_kdf_salt("default", &salt).await.unwrap();
    s.upsert_space_kdf_salt("default", &[9u8; 16])
        .await
        .unwrap();
    assert_eq!(
        s.space_kdf_salt("default").await.unwrap(),
        Some(salt),
        "已存在的盐不被覆盖（first-write-wins）"
    );
    // 随机盐 → master → kek_hash 登记全链路
    let master = crypto::argon2_master_key_with_salt(
        "abandon ability able about above absent absorb abstract absurd abuse access accident",
        &salt,
    );
    let kek = kek_hash_of(&master);
    s.register_space_crypto("default", &kek, "xchacha20-blake3-v1")
        .await
        .unwrap();
    let row = s.space_crypto("default").await.unwrap().unwrap();
    assert_eq!(row.kek_hash, kek);
    assert_eq!(row.kdf_salt.as_deref(), Some(salt.as_slice()));
}

#[allow(dead_code)]
fn _unused_hash(_: Hash) {}

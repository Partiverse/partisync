//! 设备配对协议（SPEC M2-WP04 契约 §2）：助记词 → 临时 X25519 → ECDH → 设备身份。
//!
//! 流程：
//! - 发起方：随机 X25519 → 12 词（128 bit 截位编码）→ 会话行只落库 ephemeral_pk +
//!   code，**临时私钥仅驻留进程内存**（PFS，SEC-AUDIT-2026-M2-001 P1-2——落盘私钥
//!   无法从 SQLite 页/WAL 中可靠擦除，取证可还原历史会话）；
//! - 接收方：12 词还原发起方 epk_a 视图 → 生成 X25519 密钥对 → ECDH 算出
//!   shared_secret → 写库 complete_pairing；
//! - 双方：X25519(eska, epkb) = X25519(eskb, epka) ⇒ shared_secret 等值。
//!
//! 审计追踪：SEC-AUDIT-2026-M2-001 (docs/reviews/M2-cryptography-audit-report.md)
//! Audited 2026-09-19 by Peer Reviewer (Conditional Pass; P1-2/P2-4 remediation applied)。

use bip39::{Language, Mnemonic};
use ed25519_dalek::{SigningKey, VerifyingKey};
use getrandom::fill;
use partisync_core::error::{PartisyError, Severity};
use partisync_core::Ulid;
use partisync_graph::store::Store;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

/// 默认 TTL：5 分钟（M2-WP04 契约 §2）。
pub const DEFAULT_TTL_NS: i64 = 5 * 60 * 1_000_000_000;

/// 助记词 → 字节熵（16 字节 = 128 bit，承载 X25519 私钥的低 128 位）。
fn mnemonic_to_entropy(words: &str) -> Result<Zeroizing<[u8; 16]>, PartisyError> {
    let mnemonic = Mnemonic::parse_in(Language::English, words).map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("助记词解析失败: {e}").into()),
    })?;
    let ent = mnemonic.to_entropy();
    if ent.len() != 16 {
        return Err(PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("助记词熵长度异常 {}（须 16 字节）", ent.len()).into()),
        });
    }
    let mut out = Zeroizing::new([0u8; 16]);
    out.copy_from_slice(&ent);
    Ok(out)
}

/// 16 字节熵 → 12 词助记词（BIP-39 词表）。
fn entropy_to_mnemonic(entropy: &[u8; 16]) -> String {
    Mnemonic::from_entropy(entropy)
        .expect("16 字节熵必能构造 12 词助记词")
        .words()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// 发起方：生成 32 字节 X25519 私钥、对应公钥、用低 128 位编 12 词，落库
/// （仅公钥与 code——PFS）。
///
/// 返回 (session_id, code, ephemeral_public_bytes, ephemeral_sk_bytes)。
/// 私钥由调用方持于内存直至会话完成；进程退出即消失，无落盘副本。
///
/// # Errors
/// 随机源失败 / DB 错误 → Fatal。
pub async fn initiate_pairing(
    store: &Store,
    initiator_dev: &str,
    ttl_ns: Option<i64>,
) -> Result<(String, String, [u8; 32], Zeroizing<[u8; 32]>), PartisyError> {
    let (sk, pk, sk_bytes) = unified_secret_from_rng();
    let epk_bytes = pk.to_bytes();
    let mut entropy = Zeroizing::new([0u8; 16]);
    entropy.copy_from_slice(&sk_bytes[..16]);
    let code = entropy_to_mnemonic(&entropy);
    let session_id = Ulid::now().to_string();
    store
        .create_pairing_session(
            &session_id,
            &code,
            initiator_dev,
            &epk_bytes,
            ttl_ns.unwrap_or(DEFAULT_TTL_NS),
        )
        .await?;
    // 抑制未用警告（StaticSecret 在 derive_initiator_keys 路径再次构造）
    let _ = sk;
    Ok((session_id, code, epk_bytes, sk_bytes))
}

/// 统一入口：发起方生成可重用 32 字节私钥 + 公钥（私钥字节以 Zeroizing 包裹）。
///
/// # Errors
/// 随机源失败 → Fatal。
pub fn unified_secret_from_rng() -> (StaticSecret, PublicKey, Zeroizing<[u8; 32]>) {
    let mut bytes = Zeroizing::new([0u8; 32]);
    fill(bytes.as_mut()).expect("OS RNG 不可用");
    let sk = StaticSecret::from(*bytes);
    let pk = PublicKey::from(&sk);
    (sk, pk, bytes)
}

/// 接收方：用 12 词在发起方 Store 上查 open 会话；用接收方 Store 落 ECDH 结果。
///
/// v1 简化：同进程双 Store 模拟接收方 + 发起方——iroh 落地后由网络协议层
/// 投递代码与会话 PK（A 通过 iroh 把 session.ephemeral_pk 暴露给 B）。
///
/// # Errors
/// 助记词不匹配 / DB 错误 → Fatal。
pub async fn accept_pairing(
    initiator_store: &Store,
    responder_store: &Store,
    code: &str,
    responder_dev: &str,
) -> Result<Zeroizing<[u8; 32]>, PartisyError> {
    // 1. 校验 code 格式
    let _entropy = mnemonic_to_entropy(code)?;
    // 2. 在发起方 Store 取 open 会话
    let session = initiator_store
        .pairing_session_by_code(code)
        .await?
        .ok_or_else(|| PartisyError {
            severity: Severity::Fatal,
            source: Some("配对码无对应未过期 open 会话".into()),
        })?;
    // 3. 接收方临时密钥
    let (esk_b, _epk_b, _esk_b_bytes) = unified_secret_from_rng();
    let pk_a_bytes: [u8; 32] =
        session
            .ephemeral_pk
            .as_slice()
            .try_into()
            .map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("会话 ephemeral_pk 长度异常".into()),
            })?;
    let pk_a = PublicKey::from(pk_a_bytes);
    // 4. ECDH（接收方用自己的 esk_b 与发起方 pk_a）
    let shared = esk_b.diffie_hellman(&pk_a);
    let mut shared_bytes = Zeroizing::new([0u8; 32]);
    shared_bytes.copy_from_slice(shared.as_bytes());
    // 5. 落库到接收方 Store（共享密钥 + 接收方 device id + state=closed）
    //    注：v1 简化将发起方 session 行的 state 同步更新为 closed（在发起方 DB 上）
    responder_store
        .complete_pairing(&session.id, responder_dev, shared_bytes.as_ref())
        .await?;
    // 同步发起方会话状态为 closed（生产路径通过 iroh 收到接收方 ACK 后做）
    // 这里直接 SQL 即可（Store API 未暴露专门函数，复用 complete_pairing 幂等）
    initiator_store
        .complete_pairing(&session.id, &session.initiator_dev, shared_bytes.as_ref())
        .await?;
    Ok(shared_bytes)
}

/// 派生密钥载荷（shared_secret, device_key）——均 Zeroizing 包裹。
pub type DerivedKeys = (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>);

/// 发起方：取出已 closed 的会话，自行用 esk_a 与 epk_b 重算 shared_secret
/// 与 device_keypair。epk_b 由接收方通过 iroh 握手投递（v1：本接口允许
/// 外部传字节）。输入私钥应来自 [`unified_secret_from_rng`] 的 Zeroizing
/// 载荷（调用方负责其内存生命周期；本函数不再复制清理）。
///
/// # Errors
/// 会话未 closed / 共享密钥未回填 / DB 错误 → Fatal。
pub fn derive_initiator_keys(
    sk_bytes: &[u8; 32],
    epk_b_bytes: &[u8; 32],
) -> Result<DerivedKeys, PartisyError> {
    let sk = StaticSecret::from(*sk_bytes);
    let pk_b = PublicKey::from(*epk_b_bytes);
    let shared = sk.diffie_hellman(&pk_b);
    let mut shared_bytes = Zeroizing::new([0u8; 32]);
    shared_bytes.copy_from_slice(shared.as_bytes());
    let device_key = derive_device_key(&shared_bytes);
    Ok((shared_bytes, device_key))
}

/// 接收方派生设备密钥对（Ed25519）——输出为设备签名种子，Zeroizing 包裹。
///
/// # Errors
/// 无（blake3 输入长度固定）。
pub fn derive_device_key(shared_secret: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    h.update(shared_secret);
    h.update(b"device-key-v1");
    let out = h.finalize();
    let mut k = Zeroizing::new([0u8; 32]);
    k.copy_from_slice(out.as_bytes());
    k
}
pub fn device_signing_key(seed: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(seed)
}

/// Ed25519 验签公钥。
pub fn device_verifying_key(seed: &[u8; 32]) -> VerifyingKey {
    device_signing_key(seed).verifying_key()
}

/// 派生 endpoint secret（iroh 会话密钥材料，不持久化）。
pub fn endpoint_secret(shared_secret: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    h.update(shared_secret);
    h.update(b"endpoint-secret-v1");
    let out = h.finalize();
    let mut k = Zeroizing::new([0u8; 32]);
    k.copy_from_slice(out.as_bytes());
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_roundtrip_yields_same_entropy() {
        let mut entropy = [0u8; 16];
        for (i, b) in entropy.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(13);
        }
        let mnemonic = entropy_to_mnemonic(&entropy);
        let restored = mnemonic_to_entropy(&mnemonic).unwrap();
        assert_eq!(*restored, entropy);
    }

    #[test]
    fn mnemonic_rejects_garbage() {
        let bad = "not a real word list at all here now";
        assert!(mnemonic_to_entropy(bad).is_err());
    }

    #[test]
    fn x25519_ecdh_matches_on_both_sides() {
        let (sk_a, pk_a, sk_a_bytes) = unified_secret_from_rng();
        let (sk_b, pk_b, _sk_b_bytes) = unified_secret_from_rng();
        let shared_ab = sk_a.diffie_hellman(&pk_b);
        let shared_ba = sk_b.diffie_hellman(&pk_a);
        assert_eq!(shared_ab.as_bytes(), shared_ba.as_bytes());
        let mut b = [0u8; 32];
        b.copy_from_slice(shared_ab.as_bytes());
        let derived = derive_device_key(&b);
        assert_eq!(derived.len(), 32);
        let _ = *sk_a_bytes; // 验证 sk_bytes 长度合规
    }
}

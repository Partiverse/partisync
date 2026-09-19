//! E2EE 加密栈（SPEC M2-WP07 契约 §4）：空间密钥层次 + XChaCha20-Poly1305 AEAD。
//!
//! KDF 链：master_key（Argon2id）→ space_key（HKDF-blake3 / derive_key）→
//!   content_key（per-content）/ meta_key（per-space）。
//!
//! 选型依据 ADR-0010：RustCrypto 生态（x25519-dalek / ed25519-dalek /
//! chacha20poly1305 / blake3 / argon2 / zeroize）。
//!
//! 审计追踪（M2 G3 硬门禁准备）：
//! - 审计编号: SEC-AUDIT-2026-M2-001 (docs/reviews/M2-cryptography-audit-report.md)
//! - 审计基准: RFC 9106 (Argon2id), BLAKE3 Key Derivation Specification, RFC 8439 (AEAD)
//! - 状态: Audited 2026-09-19 by Peer Reviewer (Conditional Pass; P1 remediation applied)

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use partisync_core::error::{PartisyError, Severity};
use zeroize::{Zeroize, Zeroizing};

/// 派生主密钥（Argon2id，m=64MiB/t=3/p=1）——参数在 §5.11 既定 + ADR-0010。
///
/// mnemonic：12 词助记词字符串；space_id：空间标识（作 salt 域分离来源）。
/// 返回值经 `Zeroizing` 包裹，防止栈拷贝与异步逃逸残留敏感密钥。
///
/// ⚠️ salt = blake3(space_id) 截位——对固定 space_id（如 "default"）全局一致，
/// 仅限测试与无状态派生。生产空间必须走 [`argon2_master_key_with_salt`] +
/// [`crate::crypto::random_kdf_salt`] 生成的持久随机盐（SEC-AUDIT P2-3）。
pub fn argon2_master_key(mnemonic: &str, space_id: &str) -> Zeroizing<[u8; 32]> {
    let params = Params::new(64 * 1024, 3, 1, Some(32)).expect("固定参数合法");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    // salt = blake3(space_id) ——固定 16 字节 = blake3 输出截位
    let mut salt = [0u8; 16];
    let mut h = blake3::Hasher::new();
    h.update(space_id.as_bytes());
    h.update(b"|master-salt-v1");
    let full = h.finalize();
    salt.copy_from_slice(&full.as_bytes()[..16]);
    argon
        .hash_password_into(mnemonic.as_bytes(), &salt, out.as_mut())
        .expect("Argon2 32 字节输出固定");
    salt.zeroize();
    out
}

/// BLAKE3 官方密钥派生模式（SEC-AUDIT-2026-M2-001 P1-1 整改）。
///
/// 取代原自研两阶段 keyed-hash 构造，直接使用经形式化验证的 `blake3::derive_key`。
/// 遵循 BLAKE3 规范约定：`context` 必须是协议级常量（域分离载体），
/// 变长输入（master/space_id/content_id）全部走 `key_material`。
pub fn derive_blake3_key(context: &str, key_material: &[u8]) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(blake3::derive_key(context, key_material))
}

/// space_key = blake3-derive("partisync space-key-v1", master || space_id)。
pub fn derive_space_key(master: &[u8; 32], space_id: &str) -> Zeroizing<[u8; 32]> {
    let mut material = Vec::with_capacity(32 + space_id.len());
    material.extend_from_slice(master);
    material.extend_from_slice(space_id.as_bytes());
    derive_blake3_key("partisync space-key-v1", &material)
}

/// content_key = blake3-derive("partisync content-key-v1", space_key || content_id)。
pub fn derive_content_key(space_key: &[u8; 32], content_id: &str) -> Zeroizing<[u8; 32]> {
    let mut material = Vec::with_capacity(32 + content_id.len());
    material.extend_from_slice(space_key);
    material.extend_from_slice(content_id.as_bytes());
    derive_blake3_key("partisync content-key-v1", &material)
}

/// meta_key = blake3-derive("partisync meta-key-v1", space_key)。
pub fn derive_meta_key(space_key: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    derive_blake3_key("partisync meta-key-v1", space_key)
}

/// 生成 16 字节 CSPRNG KDF 盐（SEC-AUDIT P2-3：生产空间必须持随机持久盐）。
#[must_use]
pub fn random_kdf_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).expect("OS RNG 不可用");
    salt
}

/// 以显式盐派生主密钥——生产路径（盐经 [`crate::random_kdf_salt`] 生成并由
/// `space_crypto.kdf_salt` 持久化，解锁时读回）。
pub fn argon2_master_key_with_salt(mnemonic: &str, salt: &[u8; 16]) -> Zeroizing<[u8; 32]> {
    let params = Params::new(64 * 1024, 3, 1, Some(32)).expect("固定参数合法");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(mnemonic.as_bytes(), salt, out.as_mut())
        .expect("Argon2 32 字节输出固定");
    out
}

/// XChaCha20-Poly1305 加密（返回 ciphertext + 24 字节 nonce）。
#[allow(deprecated)]
pub fn encrypt_content(key: &[u8; 32], plaintext: &[u8]) -> (Vec<u8>, [u8; 24]) {
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).expect("OS RNG 不可用");
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .expect("AEAD 加密不应失败");
    (ct, nonce)
}

/// XChaCha20-Poly1305 解密。
///
/// # Errors
/// 密文篡改 / nonce 错配 → AEAD 错误包为 Fatal。
#[allow(deprecated)]
pub fn decrypt_content(
    key: &[u8; 32],
    nonce: &[u8; 24],
    ciphertext: &[u8],
) -> Result<Vec<u8>, PartisyError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("AEAD 解密失败（密文篡改 / nonce 错配）: {e}").into()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkdf_deterministic() {
        let master = [42u8; 32];
        let k1 = derive_content_key(&master, "content-A");
        let k2 = derive_content_key(&master, "content-A");
        assert_eq!(k1, k2);
        let k3 = derive_content_key(&master, "content-B");
        assert_ne!(k1, k3, "不同 info 域分离");
    }

    #[test]
    fn content_and_meta_keys_distinct() {
        let space = [7u8; 32];
        let ck = derive_content_key(&space, "content-X");
        let mk = derive_meta_key(&space);
        assert_ne!(ck, mk, "content/meta 域分离");
    }

    #[test]
    fn argon2_master_key_deterministic() {
        let a = argon2_master_key("a b c d e f g h i j k l", "space-1");
        let b = argon2_master_key("a b c d e f g h i j k l", "space-1");
        assert_eq!(a, b);
        let c = argon2_master_key("a b c d e f g h i j k l", "space-2");
        assert_ne!(a, c, "space_id 影响 salt ⇒ 派生不同");
    }

    #[test]
    fn random_kdf_salt_is_unpredictable_and_with_salt_is_deterministic() {
        // SEC-AUDIT P2-3：随机盐不可预测；同盐派生确定、异盐派生不同
        let s1 = random_kdf_salt();
        let s2 = random_kdf_salt();
        assert_ne!(s1, s2, "两次 CSPRNG 盐不应相等");
        let m1 = argon2_master_key_with_salt("a b c d e f g h i j k l", &s1);
        let m2 = argon2_master_key_with_salt("a b c d e f g h i j k l", &s1);
        assert_eq!(m1, m2, "同盐派生必须确定（解锁路径依赖）");
        let m3 = argon2_master_key_with_salt("a b c d e f g h i j k l", &s2);
        assert_ne!(m1, m3, "异盐必须派生不同 master（抗彩虹表）");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [1u8; 32];
        let plaintext = b"hello e2ee world";
        let (ct, nonce) = encrypt_content(&key, plaintext);
        let pt = decrypt_content(&key, &nonce, &ct).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let key = [2u8; 32];
        let (mut ct, nonce) = encrypt_content(&key, b"hello");
        let n = ct.len();
        ct[n - 1] ^= 0x01;
        let err = decrypt_content(&key, &nonce, &ct).unwrap_err();
        assert!(err.to_string().contains("AEAD"));
    }

    #[test]
    fn decrypt_rejects_wrong_nonce() {
        let key = [3u8; 32];
        let (ct, mut nonce) = encrypt_content(&key, b"hello");
        nonce[0] ^= 0x01;
        assert!(decrypt_content(&key, &nonce, &ct).is_err());
    }
}

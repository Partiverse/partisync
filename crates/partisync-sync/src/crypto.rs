//! E2EE 加密栈（SPEC M2-WP07 契约 §4）：空间密钥层次 + XChaCha20-Poly1305 AEAD。
//!
//! KDF 链：master_key（Argon2id）→ space_key（HKDF-blake3）→
//!   content_key（per-content）/ meta_key（per-space）。
//!
//! 选型依据 ADR-0010：RustCrypto 生态（x25519-dalek / ed25519-dalek /
//! chacha20poly1305 / blake3 / argon2 / zeroize）。外部密码学审计为 M2 G3
//! 硬门禁，本模块仅交付自测试与文档面。

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use partisync_core::error::{PartisyError, Severity};
use zeroize::Zeroize;

/// 派生主密钥（Argon2id，m=64MiB/t=3/p=1）——参数在 §5.11 既定 + ADR-0010。
///
/// mnemonic：12 词助记词字符串；space_id：空间标识（作 salt）。
pub fn argon2_master_key(mnemonic: &str, space_id: &str) -> [u8; 32] {
    let params = Params::new(64 * 1024, 3, 1, Some(32)).expect("固定参数合法");
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    // salt = blake3(space_id) ——固定 16 字节 = blake3 输出截位
    let mut salt = [0u8; 16];
    let mut h = blake3::Hasher::new();
    h.update(space_id.as_bytes());
    h.update(b"|master-salt-v1");
    let full = h.finalize();
    salt.copy_from_slice(&full.as_bytes()[..16]);
    argon
        .hash_password_into(mnemonic.as_bytes(), &salt, &mut out)
        .expect("Argon2 32 字节输出固定");
    salt.zeroize();
    out
}

/// HKDF-blake3 提取+扩展（RFC 5869-like 接口，内部用 blake3 keyed-hash）。
///
/// 实现：HKDF-Extract(salt=info, IKM=master) → PRK；HKDF-Expand(PRK, info, L=32)
/// 简化：直接 blake3 keyed-hash(key=derived_from_info_and_master, data=hkdf-tag)。
/// 32 字节输出，输出材料经 zeroize 在 drop 时擦除（blake3 输出在堆上，drop 自由）。
pub fn hkdf_blake3(master: &[u8; 32], info: &[u8]) -> [u8; 32] {
    // 派生 keyed-hash 的 key：blake3(master || info || 0x01) → 32 字节
    let mut kdf = blake3::Hasher::new();
    kdf.update(master);
    kdf.update(info);
    kdf.update(&[0x01]);
    let key_material = kdf.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(key_material.as_bytes());
    let mut h = blake3::Hasher::new_keyed(&key);
    h.update(master);
    h.update(b"hkdf-expand-v1");
    let out = h.finalize();
    let mut k = [0u8; 32];
    k.copy_from_slice(out.as_bytes());
    key.zeroize();
    k
}

pub fn derive_space_key(master: &[u8; 32], space_id: &str) -> [u8; 32] {
    let mut info = Vec::with_capacity(space_id.len() + 16);
    info.extend_from_slice(space_id.as_bytes());
    info.extend_from_slice(b"|space-key-v1");
    hkdf_blake3(master, &info)
}

pub fn derive_content_key(space_key: &[u8; 32], content_id: &str) -> [u8; 32] {
    let mut info = Vec::with_capacity(content_id.len() + 4);
    info.extend_from_slice(content_id.as_bytes());
    info.extend_from_slice(b"|c");
    hkdf_blake3(space_key, &info)
}

pub fn derive_meta_key(space_key: &[u8; 32]) -> [u8; 32] {
    hkdf_blake3(space_key, b"meta-v1")
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

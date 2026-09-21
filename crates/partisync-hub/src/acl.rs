//! 设备会话鉴权（SPEC M3-WP03 T05，裁定 5）：ed25519 签名信封 + hub 验签
//! + 注册表角色强制。
//!
//! 信封语义：设备以 M2 pairing 派生的 ed25519 SigningKey 对
//! `space_id ␟ action ␟ nonce` 签名；hub 验签（身份 = 信封内验证密钥，
//! 注册表成员表比对）+ 角色检查（[`crate::registry::RegistryService::
//! check`] 语义）+ nonce 单调防重放。验签失败/重放/越权三种拒绝形态
//! 独立可辨。
//!
//! 强制点：数据写（write）经 [`authorize`] 后才可进入 raft 写路径；
//! 管理操作（admin，如成员变更）除信封外仍由 SM 侧 Owner 强制兜底
//! （T03 双保险）。读（read）经信封验签 + Read 角色。

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};

use crate::registry::{Action, RegistryError, RegistryService};

/// 已签名请求信封（设备 → hub；传输层编码归 T05 帧扩展）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignedRequest {
    /// 设备验证密钥（hex，64 字符）——身份。
    pub device_hex: String,
    /// 目标空间。
    pub space_id: String,
    /// 请求动作（read/write/admin）。
    pub action: String,
    /// 单调 nonce（防重放；同设备必须递增）。
    pub nonce: u64,
    /// ed25519 签名（hex，128 字符）——覆盖 `sign_bytes()`。
    pub signature_hex: String,
}

impl SignedRequest {
    /// 签名覆盖字节（编码规范：三段以 0x00 分隔）。
    #[must_use]
    pub fn sign_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(self.space_id.as_bytes());
        v.push(0x00);
        v.extend_from_slice(self.action.as_bytes());
        v.push(0x00);
        v.extend_from_slice(&self.nonce.to_be_bytes());
        v
    }

    /// 设备侧签名（demo/测试便利；生产设备用 M2 派生 SigningKey）。
    ///
    /// # Errors
    /// 身份 hex 非法。
    pub fn sign(&mut self, signing: &ed25519_dalek::SigningKey) -> Result<(), RegistryError> {
        let sig = signing.sign(&self.sign_bytes());
        self.signature_hex = hex_encode(&sig.to_bytes());
        Ok(())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 鉴权失败形态（独立可辨——审计与排障需要区分）。
#[derive(Debug)]
pub enum AuthError {
    /// 验证密钥/签名 hex 非法。
    Malformed(String),
    /// 验签失败（身份伪造或载荷被改）。
    BadSignature,
    /// nonce 未递增（重放）。
    Replay,
    /// 角色/成员规则拒绝（转发自注册表）。
    Forbidden(RegistryError),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "auth malformed: {e}"),
            Self::BadSignature => write!(f, "auth: bad signature"),
            Self::Replay => write!(f, "auth: replayed nonce"),
            Self::Forbidden(e) => write!(f, "auth: {e}"),
        }
    }
}

impl std::error::Error for AuthError {}

/// 鉴权器：nonce 单调追踪（进程内；v0.1 不持久——重放窗口限于进程
/// 生命周期，持久化防重放归 T06 会话层）。
pub struct Authorizer {
    nonces: Mutex<HashMap<String, u64>>,
}

impl Default for Authorizer {
    fn default() -> Self {
        Self {
            nonces: Mutex::new(HashMap::new()),
        }
    }
}

impl Authorizer {
    /// 新鉴权器。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 验签 + 防重放 + 注册表角色强制（registry.check 语义）。
    ///
    /// # Errors
    /// [`AuthError`] 三形态（Malformed/Replay/BadSignature）或转发
    /// `Forbidden`/`Missing`（注册表）。
    pub fn authorize(
        &self,
        registry: &RegistryService,
        req: &SignedRequest,
    ) -> Result<(), AuthError> {
        if req.signature_hex.len() != 128 {
            return Err(AuthError::Malformed("signature hex length != 128".into()));
        }
        let vk_bytes = DeviceKeyHex::decode(&req.device_hex)
            .map_err(|e| AuthError::Malformed(e.to_string()))?;
        let vk = VerifyingKey::from_bytes(&vk_bytes)
            .map_err(|e| AuthError::Malformed(format!("verifying key: {e}")))?;
        let mut sig_bytes = [0u8; 64];
        for (i, chunk) in req.signature_hex.as_bytes().chunks(2).enumerate() {
            let hi = (chunk[0] as char)
                .to_digit(16)
                .ok_or_else(|| AuthError::Malformed("signature hex digit invalid".into()))?;
            let lo = (chunk[1] as char)
                .to_digit(16)
                .ok_or_else(|| AuthError::Malformed("signature hex digit invalid".into()))?;
            sig_bytes[i] = (hi * 16 + lo) as u8;
        }
        let sig = Signature::from_bytes(&sig_bytes);
        vk.verify(&req.sign_bytes(), &sig)
            .map_err(|_| AuthError::BadSignature)?;

        // 防重放：同设备 nonce 必须严格递增
        {
            let mut nonces = self.nonces.lock().expect("nonce lock");
            let last = nonces.get(&req.device_hex).copied().unwrap_or(0);
            if req.nonce <= last {
                return Err(AuthError::Replay);
            }
            nonces.insert(req.device_hex.clone(), req.nonce);
        }

        let action = match req.action.as_str() {
            "read" => Action::Read,
            "write" => Action::Write,
            "admin" => Action::Admin,
            other => return Err(AuthError::Malformed(format!("unknown action {other}"))),
        };
        registry
            .check(&req.space_id, crate::registry::DeviceId(vk_bytes), action)
            .map_err(AuthError::Forbidden)
    }
}

/// hex 解码辅助（[`DeviceKeyHex::decode`]）。
struct DeviceKeyHex;

impl DeviceKeyHex {
    fn decode(hex: &str) -> Result<[u8; 32], String> {
        let b = hex.as_bytes();
        if b.len() != 64 {
            return Err(format!("device id hex length != 64 (got {})", b.len()));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in b.chunks(2).enumerate() {
            let hi = (chunk[0] as char)
                .to_digit(16)
                .ok_or("device id hex digit invalid")?;
            let lo = (chunk[1] as char)
                .to_digit(16)
                .ok_or("device id hex digit invalid")?;
            out[i] = (hi * 16 + lo) as u8;
        }
        Ok(out)
    }
}

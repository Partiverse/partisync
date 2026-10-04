//! 扩展装载期验签（[P21]；SPEC M9-WP04 §2.2 / ADR-0030）。
//!
//! 信任锚 = 发布双钥（[RELEASE-PUB-KEY.md]，主钥离线 + CI 子钥），
//! **任一通过即有效**。验签插桩于 [`crate::registry::ExtTool::load`]
//! 装载序 manifest 校验之后、preflight / component 编译之前——缺签/
//! 坏签在 component 字节进入 wasmtime 编译器**之前**拒绝（[P14]
//! fail-closed 同构：拒绝先于任何 host function 暴露）。
//!
//! **无豁免**：不提供 unsigned 跳过开关（env/config 均不设）——装载
//! 期强制验签字面执行（威胁模型 T-S1/T-S2，M7-WP03-enterprise-topics
//! §2，缓解优先级 P1）。签名产出 = minisign CLI（不进依赖图，ADR-0027
//! 决策 1 口径）。

use minisign_verify::{PublicKey, Signature};

/// 锚定公钥（minisign 公钥 base64 行，docs/release/RELEASE-PUB-KEY.md
/// 原样内嵌；**任一通过即有效**，沿 RELEASE-PUB-KEY.md 验签语义）。
pub const ANCHOR_PUBKEYS: [&str; 2] = [
    // 主钥（指纹 EBC32789A716D70A，生成仪式 2026-10-01，离线双人保管）
    "RWQK1xaniSfD6+Wn9Qp+/A+WIUQ4P/tUoIrtuQawXZozUeu/BjIst0IU",
    // CI 子钥（指纹 E056CBB62BF3EF34，仅 CI secret，自动化产物签名）
    "RWQ07/MrtstW4BJcQJxvDyj415FpHOt9qD6/5PDzjkQKQF/HDvxgaX4j",
];

/// 验签错误（signature 模块内部面；registry 侧映射
/// [`crate::registry::LoadError::Unsigned`] / `BadSignature`）。
#[derive(Debug)]
pub enum SignatureError {
    /// 同名 `.minisig` 缺失。
    MissingSig(String),
    /// 锚定公钥 base64 非法（产品常量路径不可达；参数化入口防呆）。
    BadAnchor(String),
    /// 签名文件格式非法，或对全部锚钥验证失败。
    BadSignature(String),
}

impl std::fmt::Display for SignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSig(m) => write!(f, "signature file missing: {m}"),
            Self::BadAnchor(m) => write!(f, "bad anchor pubkey: {m}"),
            Self::BadSignature(m) => write!(f, "bad signature: {m}"),
        }
    }
}

impl std::error::Error for SignatureError {}

/// minisign 公钥 base64 → [`PublicKey`]（0.3.0 API，vendored 实证：
/// `PublicKey::from_base64`）。
///
/// # Errors
/// base64 解码或密钥长度非法。
pub fn pubkey_from_base64(b64: &str) -> Result<PublicKey, SignatureError> {
    PublicKey::from_base64(b64).map_err(|e| SignatureError::BadAnchor(format!("decode: {e}")))
}

/// 产品锚集：[`ANCHOR_PUBKEYS`] 解析。内嵌常量打错属声明面缺陷，在本
/// crate 首次装载即暴露（expect 为显式 fail-fast，非运行期输入路径）。
#[must_use]
pub fn anchored_pubkeys() -> Vec<PublicKey> {
    ANCHOR_PUBKEYS
        .iter()
        .map(|b64| {
            pubkey_from_base64(b64)
                .expect("内嵌锚定公钥常量必须合法（RELEASE-PUB-KEY.md 原样内嵌）")
        })
        .collect()
}

/// 对 content 验签：逐锚钥验证，**任一通过即放行**（`allow_legacy=false`
/// ——0.3.0 `PublicKey::verify` 第三参，legacy 预哈希算法直接
/// `UnexpectedAlgorithm` 拒，vendored 源码实证）。
///
/// # Errors
/// 全部锚钥均验证失败（含 key id 不匹配的「未知钥签名」）。
pub fn verify(
    content: &[u8],
    sig: &Signature,
    anchors: &[PublicKey],
) -> Result<(), SignatureError> {
    let mut last = String::from("no anchor keys configured");
    for pk in anchors {
        match pk.verify(content, sig, false) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(SignatureError::BadSignature(last))
}

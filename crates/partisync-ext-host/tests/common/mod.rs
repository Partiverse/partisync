//! 集成测试共享设施（非 test target：`tests/common/mod.rs` 惯例）。
//!
//! [P21]（SPEC M9-WP04 §2.3/§4）：fixtures 签名出自 **test-only** keypair
//! （`fixtures/test-signing.pub`，pub 注释显式标注非生产钥）——产品锚
//! （内嵌发布双钥）不信任它；锚定路径的既有正路测试经
//! `load_with_anchors` / `register_with_anchors` / `scan_with_anchors`
//! 注入本钥走「测试钥签名路径」，产品入口无任何豁免。

/// test-only 钥 base64（`fixtures/test-signing.pub` 第二行原样）。
pub const TEST_PUB_B64: &str = "RWQVdtZEx/IGjqq0d6egWWBl2+3QtiS0aNQrpc/F6Tqrt5TAaOdIc72B";

/// 测试锚集（test-only 钥解析；与 fixtures 签名钥配对）。
pub fn test_anchor() -> Vec<minisign_verify::PublicKey> {
    vec![partisync_ext_host::pubkey_from_base64(TEST_PUB_B64)
        .expect("test-only pubkey base64 必须合法（fixtures/test-signing.pub 原样）")]
}

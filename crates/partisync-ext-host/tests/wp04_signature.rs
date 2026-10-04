//! [P21] 五路探针（SPEC M9-WP04 §2.2/§4）：装载期强制验签的信任面。
//!
//! fixtures 签名均出自 test-only keypair（`common::TEST_PUB_B64`，pub
//! 注释显式标注非生产钥）——它在产品锚（内嵌发布双钥）下**天然是未知
//! 钥**，故探针④（未知钥必拒）可直接用产品入口 + 入仓 fixture 闭环，
//! 生产钥材料零接触。合法通过路径（探针①）与编译对照（探针⑤反证）
//! 经 `load_with_anchors` 注入测试钥锚——验签恒强制，仅锚集来源可换
//! （SPEC §4「测试钥签名路径」，非豁免通道）。

use partisync_ext_host::registry::{ExtRegistry, ExtTool, LoadError};
use partisync_ext_host::HostState;

mod common;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wp04-sig-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

fn write_manifest(dir: &std::path::Path, stem: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(format!("{stem}.json"));
    std::fs::write(&p, body).expect("write manifest");
    p
}

const MANIFEST: &str = r#"{"tool_name":"sig_probe","capabilities":[]}"#;

/// 探针①：合法签名（测试钥锚 + 入仓 fixture 签名对）装载通过，且调用
/// 通路正常——验签放行不破坏既有装载语义。
#[test]
fn p21_1_valid_signature_loads() {
    let dir = tmp_dir("valid");
    let tool = ExtTool::load_with_anchors(
        fixture("demo_tool.wasm"),
        write_manifest(&dir, "sig_probe", MANIFEST),
        HostState::without_index(),
        &common::test_anchor(),
    )
    .expect("合法签名必须装载通过");
    let out = tool.call(r#"{"k":"v"}"#).expect("call 成功");
    assert!(out.contains("input_bytes"), "输出异常：{out}");
}

/// 探针②：篡改 `.wasm` 单字节必拒（`BadSignature`）——字节 ↔ 签名
/// 绑定，任何改动越不过验签。
#[test]
fn p21_2_single_byte_tamper_rejected() {
    let dir = tmp_dir("tamper");
    let wasm = dir.join("demo.wasm");
    std::fs::copy(fixture("demo_tool.wasm"), &wasm).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.join("demo.minisig")).unwrap();
    // 篡改文件中点一字节（翻转）
    let mut bytes = std::fs::read(&wasm).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    std::fs::write(&wasm, &bytes).unwrap();

    // 产品入口（内嵌发布双钥锚）——拒绝与锚集无关，字节绑定本身失效
    let err = ExtTool::load(
        &wasm,
        write_manifest(&dir, "demo", MANIFEST),
        HostState::without_index(),
    )
    .expect_err("篡改单字节必须拒");
    assert!(
        matches!(err, LoadError::BadSignature(_)),
        "必须 BadSignature，got {err:?}"
    );
    assert!(
        err.to_string().contains("demo"),
        "Display 携带扩展定位：{err}"
    );
}

/// 探针③：缺 `.minisig` 必拒（`Unsigned`）——无豁免通道。
#[test]
fn p21_3_missing_signature_rejected() {
    let dir = tmp_dir("unsigned");
    let wasm = dir.join("demo.wasm");
    std::fs::copy(fixture("demo_tool.wasm"), &wasm).unwrap();
    // 不拷 .minisig

    let err = ExtTool::load(
        &wasm,
        write_manifest(&dir, "demo", MANIFEST),
        HostState::without_index(),
    )
    .expect_err("缺签必须拒");
    assert!(
        matches!(err, LoadError::Unsigned(_)),
        "必须 Unsigned，got {err:?}"
    );
    assert!(
        err.to_string().contains("demo"),
        "Display 携带扩展定位：{err}"
    );
}

/// 探针④：未知钥签名必拒——产品锚（发布双钥）+ test-only 钥签名
/// fixture（对产品锚即未知钥），产品入口直测闭环。
#[test]
fn p21_4_unknown_key_signature_rejected() {
    let dir = tmp_dir("unknown-key");
    // 产品入口 + 入仓签名（test-only 钥 = 产品锚下的未知钥）
    let err = ExtTool::load(
        fixture("demo_tool.wasm"),
        write_manifest(&dir, "demo", MANIFEST),
        HostState::without_index(),
    )
    .expect_err("未知钥签名必须拒");
    assert!(
        matches!(err, LoadError::BadSignature(_)),
        "必须 BadSignature，got {err:?}"
    );
}

/// 探针⑤：拒绝先于编译——验签向拒绝变体（②③④）均非 `Component` 向
/// （wasmtime 编译器未接触字节）；反证对照：签名合法但字节非法
/// （`garbage.wasm` + 合法签名）确实到达编译器（`Component` 向可达），
/// 排除「验签层碰巧吞掉一切」的假阳性。
#[test]
fn p21_5_rejection_precedes_compilation() {
    let dir = tmp_dir("precompile");

    // 验签向三路（复用②③④形态，聚焦变体断言）
    let tamper = dir.join("t.wasm");
    std::fs::copy(fixture("demo_tool.wasm"), &tamper).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.join("t.minisig")).unwrap();
    let mut bytes = std::fs::read(&tamper).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    std::fs::write(&tamper, &bytes).unwrap();
    let unsigned = dir.join("u.wasm");
    std::fs::copy(fixture("demo_tool.wasm"), &unsigned).unwrap();

    for (name, wasm) in [
        ("tampered", tamper.as_path()),
        ("unsigned", unsigned.as_path()),
        ("unknown-key", fixture("demo_tool.wasm").as_path()),
    ] {
        let manifest = write_manifest(&dir, name, MANIFEST);
        let err =
            ExtTool::load(wasm, &manifest, HostState::without_index()).expect_err("验签向拒绝");
        assert!(
            matches!(err, LoadError::BadSignature(_) | LoadError::Unsigned(_)),
            "{name}: 拒绝必须验签向，got {err:?}"
        );
        assert!(
            !matches!(err, LoadError::Component(_)),
            "{name}: 字节不得进入 wasmtime 编译器"
        );
    }

    // 反证对照：签名合法（测试钥锚）+ 垃圾字节 → 验签放行 → 编译拒
    // （`Component` 向可达，⑤的「先于编译」非平凡成立）
    let err = ExtTool::load_with_anchors(
        fixture("garbage.wasm"),
        write_manifest(&dir, "garbage", MANIFEST),
        HostState::without_index(),
        &common::test_anchor(),
    )
    .expect_err("垃圾字节签名再合法也必须编译拒");
    assert!(
        matches!(err, LoadError::Component(_)),
        "验签放行后必须到达编译器，got {err:?}"
    );
}

/// 附加（SPEC §2.2）：scan 孤儿 `.minisig`（有签名无同名 wasm）显式拒
/// （沿孤儿 `.wasm` P2-1 判例）；三文件配对 scan 正常路径不回归。
#[test]
fn p21_scan_orphan_signature_rejected_and_paired_scan_ok() {
    let dir = tmp_dir("scan");
    // 配对扩展（demo）+ 孤儿签名（orphan.minisig 无 orphan.wasm）
    std::fs::copy(fixture("demo_tool.wasm"), dir.join("demo.wasm")).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.join("demo.minisig")).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.join("orphan.minisig")).unwrap();
    write_manifest(&dir, "demo", MANIFEST);

    let err =
        ExtRegistry::scan_with_anchors(&dir, &HostState::without_index(), &common::test_anchor())
            .expect_err("孤儿 .minisig 必须显式拒");
    assert!(
        matches!(err, LoadError::Scan(ref m) if m.contains("orphan signature")),
        "必须 Scan(orphan signature)，got {err:?}"
    );

    // 移除孤儿后 scan 正常（配对路径不回归）
    std::fs::remove_file(dir.join("orphan.minisig")).unwrap();
    let reg =
        ExtRegistry::scan_with_anchors(&dir, &HostState::without_index(), &common::test_anchor())
            .expect("配对 scan 必须成功");
    assert_eq!(reg.len(), 1);
}

//! T02 集成探针：[P14] 注权 manifest 不变量（SPEC M7-WP01 §2.2 + §3）。
//!
//! 覆盖三类加载期拒绝（拒绝先于任何 host function 暴露）：
//! 1. 缺 manifest —— `Manifest::load` 对应 IO 失败
//! 2. 撞名 —— `ValidateError::ToolNameCollision`（内建优先）
//! 3. 缺/篡改 manifest —— 同上 Manifest::load 路径

use std::io::Write;
use std::path::PathBuf;

use partisync_ext_host::{Capability, Manifest, ManifestError, ValidateError};
use tempfile::tempdir;

fn write_manifest(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(format!("{name}.json"));
    let mut f = std::fs::File::create(&p).expect("create manifest");
    f.write_all(body.as_bytes()).expect("write manifest");
    p
}

#[test]
fn p14_missing_manifest_loading_fails() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");
    let err = Manifest::load(&path).expect_err("缺 manifest 必须失败");
    assert!(matches!(err, ManifestError::Io(_)));
}

#[test]
fn p14_builtin_name_collision_rejected() {
    let dir = tempdir().unwrap();
    for builtin in ["asset_read", "search", "cas_stats"] {
        let path = write_manifest(
            dir.path(),
            builtin,
            &format!(r#"{{"tool_name":"{builtin}","capabilities":["clock.read"]}}"#),
        );
        let err = Manifest::load(&path).expect_err("撞名必须拒绝");
        match err {
            ManifestError::Validate(ValidateError::ToolNameCollision(n)) => assert_eq!(n, builtin),
            other => panic!("期望撞名拒绝，得到 {other:?}"),
        }
    }
}

#[test]
fn p14_unknown_capability_rejected_at_parse() {
    let dir = tempdir().unwrap();
    let path = write_manifest(
        dir.path(),
        "evil",
        r#"{"tool_name":"evil","capabilities":["fs.read","net.connect"]}"#,
    );
    let err = Manifest::load(&path).expect_err("fs.read/net.connect 不在白名单必须拒绝");
    match err {
        ManifestError::Parse(msg) => {
            assert!(msg.contains("fs.read") || msg.contains("net.connect"))
        }
        other => panic!("期望 Parse 拒绝，得到 {other:?}"),
    }
}

#[test]
fn p14_bad_tool_name_rejected() {
    let dir = tempdir().unwrap();
    for bad in ["", "has space", &"x".repeat(200)] {
        let path = write_manifest(
            dir.path(),
            "tmp",
            &format!(r#"{{"tool_name":"{bad}","capabilities":[]}}"#),
        );
        let err = Manifest::load(&path);
        assert!(
            matches!(
                err,
                Err(ManifestError::Validate(ValidateError::BadToolName))
            ),
            "bad 工具名 `{bad}` 应触发 BadToolName，得到 {err:?}"
        );
    }
}

#[test]
fn p14_valid_manifest_round_trip() {
    let dir = tempdir().unwrap();
    let path = write_manifest(
        dir.path(),
        "good",
        r#"{"tool_name":"my_ext","capabilities":["clock.read","clock.read"]}"#,
    );
    let m = Manifest::load(&path).expect("合法 manifest 应通过");
    assert_eq!(m.tool_name, "my_ext");
    // 反序列化保序；去重语义在 validate 内部
    assert_eq!(
        m.capabilities,
        vec![Capability::ClockRead, Capability::ClockRead]
    );
    assert_eq!(m.validate(), Ok(()));
}

#[test]
fn p14_capability_display_round_trip() {
    // [P14] 公共表示稳定：Display / as_str 出点号（SPEC §2.2 表对齐）
    assert_eq!(Capability::IndexRead.as_str(), "index.read");
    assert_eq!(Capability::ClockRead.as_str(), "clock.read");
    assert_eq!(format!("{}", Capability::ClockRead), "clock.read");
}

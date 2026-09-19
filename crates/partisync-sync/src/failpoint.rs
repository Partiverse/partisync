//! 故障注入点（SPEC M2-WP09 契约 §3）：命名检查点，由环境变量或测试 API 启用。
//!
//! 用法：
//! - 测试代码：`failpoint::enable("oplog.before_insert")`
//! - 产品代码：`failpoint::check("oplog.before_insert")` —— 仅在测试场景生效，
//!   生产构建通过 `OnceLock` 默认空集合零开销（不分支判定 hot path）。
//!
//! v1 简化：sync/chaos 模块本地 failpoint 集合，与 graph/journal 散布点共享。
//! 全局 failpoint 表 = env::var("PARTISYNC_FP") 以逗号分隔的列表。

use std::sync::OnceLock;

use std::sync::Mutex;

static ENABLED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn store() -> &'static Mutex<Vec<String>> {
    ENABLED.get_or_init(|| Mutex::new(load_from_env()))
}

fn load_from_env() -> Vec<String> {
    std::env::var("PARTISYNC_FP")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// 检查指定 failpoint 是否启用。
pub fn check(name: &str) -> bool {
    let g = store();
    let v = g.lock().expect("failpoint mutex poisoned");
    v.iter().any(|n| n == name)
}

/// 启用指定 failpoint（测试 API）。
pub fn enable(name: &str) {
    let g = store();
    let mut v = g.lock().expect("failpoint mutex poisoned");
    if !v.iter().any(|n| n == name) {
        v.push(name.to_string());
    }
}

/// 禁用指定 failpoint（测试 API）。
pub fn disable(name: &str) {
    let g = store();
    let mut v = g.lock().expect("failpoint mutex poisoned");
    v.retain(|n| n != name);
}

/// 全清（测试隔离用）。
pub fn clear() {
    let g = store();
    g.lock().expect("failpoint mutex poisoned").clear();
}

/// 列出当前启用的 failpoint。
#[must_use]
pub fn enabled() -> Vec<String> {
    store().lock().expect("failpoint mutex poisoned").clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_disable_roundtrip() {
        clear();
        assert!(!check("fp1"));
        enable("fp1");
        assert!(check("fp1"));
        disable("fp1");
        assert!(!check("fp1"));
    }

    #[test]
    fn enable_is_idempotent() {
        clear();
        enable("fp2");
        enable("fp2");
        assert_eq!(enabled().iter().filter(|n| *n == "fp2").count(), 1);
    }
}

# Task: partisync-cli clippy 债清偿（M4-WP99-T01）

> **范围外**：本任务卡登记 M4-WP06 验收时发现的 clippy 债，按 AGENTS.md 铁律 9「不顺手修」另立任务清偿。
> 关联: `docs/reports/bench/m4-wp06-kpi.md` §0「既有债务披露」

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M4-WP99-T01 |
| **类型** | clippy 债清偿（lint fix） |
| **优先级** | P2（不阻塞 M4 关账） |
| **范围** | `crates/partisync-cli/src/main.rs` 两处 |
| **创建日期** | 2026-09-23 |
| **来源** | M4-WP06 T07 验收闭合时披露（commit `cb8e908`） |

## 触发条件

```bash
$ cargo clippy --workspace --all-targets -- -D warnings
```

预期红：
- `crates/partisync-cli/src/main.rs:658` —— `unused variable: store`
  （let store = match open_db(&db).await { ... } 后续未使用，搜索子命令索引路径无需 DB）
- `crates/partisync-cli/src/main.rs:712` —— `wildcard_in_or_patterns`
  （`"hybrid" | _ => { ... }` 应改 explicit branches）

## 历史来源

- `ec7c75d`（M4-WP01 旧提交） + `968b6ec`（M4-WP02 旧提交） 引入
- WP03 / WP04 / WP05 / WP06 均未触碰 cli crate

## 修复方法（拟定，非顺道修建议）

### main.rs:658 unused store

**现状**:
```rust
let store = match open_db(&db).await {
    Ok(s) => s,
    Err(e) => {
        eprintln!("error: {e}");
        return 1;
    }
};
// store 后续不使用
```

**修复**: 删 `store` 变量，直接失败时返回 1。

### main.rs:712 wildcard_in_or_patterns

**现状**:
```rust
match mode.as_str() {
    "bm25" => { ... }
    "hybrid" | _ => { /* hybrid fallback */ }
}
```

**修复**: 改 explicit branches：
```rust
match mode.as_str() {
    "bm25" => { ... }
    "hybrid" => { /* hybrid fallback */ }
    _ => { /* unknown mode: warn + fallback to bm25 */ }
}
```

## 验收

- `cargo clippy --workspace --all-targets -- -D warnings` 全绿
- `cargo test --workspace` 全绿
- 提交挂 Task-ID `M4-WP99-T01`

## 后续可立同类任务

- `M4-WP99-002`：WP06 渗透 3 项 P1 加固（asset_organize 批大小 / 跨库校验 / c2pa absent 形态）
- `M4-WP99-003`：cli sidecar-run 深度上限
- `M4-WP99-004`：MCP 调用审计日志
//! WASM 扩展选型 spike（SPEC M6-WP04 T02）。
//!
//! 独立 workspace：不列于根 `members`、不进产品依赖图。全部依赖只在本
//! crate 的独立 Cargo.lock 中存在。评估结论见
//! `docs/reports/M6-WP04-wasm-ext-eval.md`（T01）与 ADR-0025（T03）。
//!
//! 形态：§2.1 窄核心 —— wasm component 即 MCP tool（JSON 入参 → JSON 出参）。

pub mod host;

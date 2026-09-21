# ADR-0014: pack 纠删码采用 reed-solomon-erasure

状态: 已接受 · 日期: 2026-09-21 · 决策人: @lead（AI 代理起草，用户已签核
2026-09-22）· 关联: SPEC M3-WP04（裁定 2）·
替代方案: reed-solo­mon-simd、自研 GF(256)

## 背景

数据面 pack v2 需要 RS(10,4) 条带纠删：10 数据 + 4 校验分片，单 pack
损坏 ≤4 分片可重建（SPEC M3-WP04 裁定 2）。工作区全局禁止 unsafe
（根 AGENTS/执行方案 §5.2），需要纯 safe 的 RS 实现。

## 决策

采用 `reed-solomon-erasure`（crates.io，Backblaze backblaze_erasure_
clone 的 Rust 移植，VMware 开源维护）：

- **消费面无 unsafe**：工作区代码零 unsafe（进场核实：crate 内部
  galois 实现含封装的 unsafe 指针运算，但对外 API 无 unsafe 边界要求
  ——与 reed-solomon-simd 的显式 SIMD/unsafe 路线相对）；
- **API 形态**：`ReedSolomon::new(data, parity)` + `encode`/`reconstruct`，
  支持按 Option 分片的重建语义——恰好匹配「分片存在位图」的 pack 修复
  模型；
- **版本**：`"6"`（进场时以 docs.rs 当前 6.x 为准并锁定 Cargo.lock）；
- **性能口径**：纯 safe 实现 SIMD 化不足，吞吐低于 simd 版——本 WP 的
  EC 编解码只发生在 pack 落盘/修复路径（非读路径热区），基准实测数字
  归 m3-wp04-kpi.md 登记；若实测成为瓶颈，修订本 ADR 换 simd 版并
  单独走 unsafe 放宽评审。

## 后果

- crates/partisync-cas 增 `ed25519-dalek` 式同源依赖一条
  （reed-solomon-erasure "6"），deny.toml 复核零新增白名单外条目；
- EC 只在 T02 指定路径使用（pack 编解码），不进入元数据/复制面；
- 分片数固定 10+4（硬编码于 ec.rs，成常数并测试钉住）；变更分片数
  = pack 版本升级，需新 ADR。

## Transitive 风险附注（2026-09-21 追加）

进场实测：cargo deny check 在 `advisories` 维度报
RUSTSEC-2024-0384（`instant v0.1.13` unmaintained），依赖链：

```
reed-solomon-erasure v6 → parking_lot v0.11.2 → instant v0.1.13
```

风险评估：

- `instant` 仅 `unmaintained`，无已知安全漏洞；advisory 推荐迁移至
  `web-time`，但 `parking_lot v0.11.x` 仍硬依赖 `instant`（v0.12+
  才切走），`reed-solomon-erasure v6` 锁 `parking_lot v0.11`。
- 升级路径：等 `reed-solomon-erasure` 升 `parking_lot ≥ 0.12` 后再跟进
  （跟踪 upstream issue；当前 v6 main 暂未发版）。届时本附注作废，
  deny.toml `ignore` 同步撤销。

处置：deny.toml `[advisories]` 增加 `ignore = ["RUSTSEC-2024-0384"]`，
引用本 ADR。本豁免范围限于 `instant` 一条，其他 advisory 一律拦截。

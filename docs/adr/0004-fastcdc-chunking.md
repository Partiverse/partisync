# ADR-0004: CAS 分块引入 fastcdc（v2020）与块索引复用 sqlx

状态: 已接受 · 日期: 2026-09-18 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M0-WP03、ADR-0002、调研方案 §5.9

## 背景

块级去重与 delta（PartiSync 对云端 rclone 的核心差异化，调研方案 §2.2/§5.8）需要内容定义分块（CDC）。
调研方案 §6 既定 fastcdc 5.0。

## 决策

1. **分块器 = fastcdc::v2020**（Nathan Fiedler 的 FastCDC 2020 规范实现）：
   API 经源码核验——`FastCDC::new(&[u8], min, avg, max)`，迭代产出 `Chunk{hash,offset,length}`；
   **约束：min/avg/max 须为 8 的倍数**（v2020 gear 表设计），包装层负责归一化并文档化。
2. **生产参数**：min 256 KiB / avg 1 MiB / max 4 MiB（restic/borg/kopia 经验区间，
   调研方案 §5.9）；测试参数 64 B / 256 B / 1 KiB（属性测试速度）。
3. **块索引复用 sqlx/SQLite**（块哈希 PK + 引用计数）：块索引是元数据（L0 语义），
   不为它引入 redb——redb 引入推迟到出现「设备侧无 SQLite 依赖的独立进程」需求时再评估。

## 备选

- 自写 FastCDC（Rabin 滚动）：密码学邻接轮子，否决；
- buzhash/casync 系：无更优维护性的 Rust 实现，否决；
- redb 做块索引：额外依赖 + 双引擎运维，SQL 引用计数已够 v1，推迟。

## 后果

- 分块确定性由 fastcdc 保证 + P1–P3 属性测试钉住；
- v2020 的 8 倍数约束成为 `CdcConfig` 的不变量（构造函数归一化 + debug_assert）。

## 重新评估条件

- fastcdc 停更 >1 年或出现正确性缺陷 → 自写或换 rabin 分块（P1–P3 测试保护迁移）。

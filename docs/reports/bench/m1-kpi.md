# M1 KPI 达标报告

- 日期：2026-09-19 · 本机 Apple Silicon · 对照调研方案 §7 M1 列

## KPI ①：消费 10⁴ 对象 < 2 分钟

- 方法：`gen-fixture --files 10000`（3003 重复）→ 自家 S3 网关（HTTP 全协议栈）
  → `index-remote --scheme s3 --bucket kpi10k`（fresh db，含作业 checkpoint 落表）
- **实测：42.2s（10000 对象 / 312.3MB 哈希）**——目标 120s，余量 2.8× ✅

## KPI ②：LIST p99 < 50ms（元数据引擎口径）

- 库：M0 的 10⁶ 条目库（目标口径 10⁷——当前库 10⁶，10⁷ 归 M3 规模验证，诚实标注）
- 实测演变（三个发现，全部留痕）：
  1. 初测 p99=3795ms——**测的是 16.6 万子项目录的 JSON 全量序列化**（病态样本混入）；
  2. `/api/list` 加 LIMIT 1000（UI 分页语义，真缺口）→ 仍 538-712ms；
  3. 根因 = 单列 parent_id 索引下 ORDER BY 全排序 → **schema v5 复合索引
     `(parent_id, kind DESC, name)`**（只增迁移）→ **49.4ms（11×）** ✅
- 典型目录（≤1000 子项）：0.8–1.1ms ✅

## KPI ③：互操作矩阵（M1 头号 DoD）

- `scripts/interop.sh`：**15 PASS / 0 FAIL**（S3 面 7 + WebDAV 面 4 + 消费面 2 + 跨协议 1 + 列桶 1）
- CI job 已接线（D10 计费解除后自动生效）

## 伴随改进

- index-remote 作业化（checkpoint/resume）+ `--bwlimit` 令牌桶限速（1M/s 实测精确生效）；
- 自研 OpenDAL HTTP 传输（绕过 0.59.2 打包缺陷）+ region/凭证防御性默认。

## 遗留

- 10⁷ 条目 LIST 口径（M3）；S3 网关 SigV4 验证；WebDAV Lock/目录 MOVE；
- index-remote 的 `--resume-job` 在 stop_after 注入外的真中断场景（kill -INT）待补自动验证。

# Task: M9-WP02-T05 CAS exists() 精化（M8 微债清偿，WP02 关账件）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP02-T05 |
| **类型** | 微债清偿（错误面精化；R0 → 全审逐行极小 diff） |
| **优先级** | P1（WP02 收尾；SPEC §2.6 唯一遗留实施项） |
| **范围** | SPEC M9-WP02 §2.6 + §5 清单 tier.rs + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP02 批准（PR #122）+ T04 收官（#129/#130/#131 合入，main=`6de5217`） |

## 交付物

1. **`FsBackend::exists` 精化**（tier.rs:156）：仅 `NotFound` →
   `Ok(false)`；其他 stat 错误（ENOTDIR/EACCES/IO）→ `Err(TierError::Io)`
   显式上浮——旧行为 `is_file()` 把「查不动」伪装「不存在」。
2. **`crash_resume` 调用方**（tier.rs:549）：`.unwrap_or(false)` 移除，
   `?` 上浮——stat 错误时不再误判「目标缺」而错误回滚迁移。
3. **错误注入探针**：`exists_stat_error_surfaced_not_disguised`——扇出
   首段路径（root/k1）被同名文件占用 → 同前缀 key stat 报 ENOTDIR，
   断言 `Err(Io)` 且 kind ≠ NotFound（证明 stat 错误 ≠ NotFound 面，
   SPEC §3 验收）；既有 crash_resume 双用例零回归。

## 验收

- [x] 错误注入用例证明 stat 错误 ≠ NotFound 面；
- [x] 既有 tier 测试零回归（crash_resume 完成/回滚两用例绿）；
- [x] fmt/clippy/test 三件套全绿；零新增依赖；
- [x] 提交挂 Task-ID `M9-WP02-T05`。

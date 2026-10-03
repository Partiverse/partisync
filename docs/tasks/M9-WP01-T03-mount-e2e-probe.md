# Task: M9-WP01-T03 容器真挂载 `--graph` e2e 探针（T03）

> **范围外**：F1 sync_stats 派生已随 T04（PR #119）交付；本卡不触碰
> desktop 面。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP01-T03 |
| **类型** | 测试（环境门控真挂载探针）+ 实施期缺陷修复 |
| **优先级** | P0（SPEC §3 首条验收：真挂载端到端） |
| **范围** | `crates/partisync-gateway/tests/wp01_mount_wiring.rs`（新）+ `crates/partisync-gateway/tests/wp01_wiring.rs`（diag 对照）+ `crates/partisync-fuse/src/fs.rs`（create 事件路径修复）+ `crates/partisync-gateway/src/wiring.rs`（store 访问器/applied 计数）+ `crates/partisync-graph/src/store.rs`（wal_checkpoint）+ 本卡 |
| **创建日期** | 2026-10-03 |
| **来源** | SPEC M9-WP01 §3 首条；容器配方 = `rust:1.94-alpine`（M8-WP07-T01 判例：`-j 1` / `CARGO_TARGET_DIR` 隔离 / `apk add g++ cmake make`）+ `--device /dev/fuse --cap-add SYS_ADMIN` |

## 交付物

1. **真挂载 e2e 探针**（`wp01_mount_wiring.rs`，环境门控）：backing 预置
   → `with_wiring` 挂载 → VFS create/rename/unlink → 事件经装配层进
   graph/oplog → **applied 计数排空等待**（纯原子）→ 写方 `wal_checkpoint`
   → A 侧终态/同根（P19-a）→ 手动 push 第二端可见 + size 一致 + 不动点。
   容器实测 **PASS（0.20s，确定性）**；无 FUSE 环境 SKIP 留痕。
2. **实施期缺陷修复（探针抓出）**：create 钩子事件路径为绝对路径——
   `backing_path()` 返回绝对路径（M8 时代直写 fs 恰好兼容的隐疾），strip
   backing 前缀还原相对口径；修前探针抓出「绝对路径事件把整条目录链灌进
   graph」（跨层缺陷只有 e2e 探针能抓，M8-WP07-T02 判例再现）。
3. **配套 API**：`Store::wal_checkpoint()`（TRUNCATE 合回主库；写方主动
   checkpoint = 跨连接可见性的确定性保证）；`WiringSession::store()` /
   `applied_counter()`（排空等待面）。
4. **diag 对照测试**（`wp01_wiring.rs`）：tick(bisync) 并发不破坏外部
   可见性（定位期钉面用，保留为回归）。

## 定位过程判例（十轮容器，如实登记）

- run1（绝对路径版）根断言失败 → 叶 dump 抓出 create 绝对路径缺陷；
- run3-14（修复后）转出「serve 自见、外部 pool 十轮不可见/60s 窗抖动」
  ——逐项排除 overlayfs（TMPDIR 对照）、tick 并发（diag 对照）、快照
  陈旧（fresh pool），终以 **写方 checkpoint** 定为确定性保证；
- serve 不可 `await` 终止（fuse 持 tx 克隆永不关闭通道）——排空等待改
  applied 原子计数；
- Tauri/PartiFuse 内嵌 tokio Runtime 不可在 async 上下文 drop——挂载
  句柄归还 blocking 线程（T04 卡已记，本卡实证）。

## 验收

- [x] 容器真挂载探针 PASS（0.20s；M8 判例容器复测路径）；
- [x] 本地 fmt/clippy 零警告 + wp01_wiring 5/5 + gateway 24 单测绿；
- [ ] CI 绿合入（CI runner 无 FUSE → 探针 SKIP，回归面照常跑）。

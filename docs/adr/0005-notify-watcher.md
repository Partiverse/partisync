# ADR-0005: 文件事件引入 notify（8.x）并自写去抖合并

状态: 已接受 · 日期: 2026-09-18 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M0-WP04、调研方案 §6/§5.7

## 背景

增量事件驱动是万亿级推算下的必需品（全量扫描 10¹² 条目 ≈3.2 年，调研方案 §4.1）。
本地 fs 事件源需要跨平台抽象（inotify/FSEvents/ReadDirectoryChangesW）。

## 决策

1. **事件源 = notify 8.2**（调研方案 §6 既定；155M 下载，跨平台事实标准）。
   网络/虚拟 fs 上事件不可靠 → 扫描兜底（WP07 周期性对账）。
2. **去抖合并自写**（tokio 时间窗批处理），不引入 notify-debouncer-*：
   合并语义（按 path 去重、取末态）与 PartiGraph 的应用逻辑强相关，自写 30 行以内且可测；
   debouncer 库的线程模型与我们的 journal 持久化点耦合不上。
3. **事件先落 journal 再应用**（P8 崩溃一致性口径）：应用是幂等 upsert/delete，
   崩溃后重启重放 pending 事件即收敛；journal 行应用后删除（v1 队列语义）。
4. 归属：`partisync-graph::watch`（M2 若随设备同步迁移到 sync crate，需 ADR 记录）。

## 备选

- notify-debouncer-full：见决策 2，否决；
- 轮询扫描替代事件：量级不成立（§4.1），仅作兜底而非主路径；
- kqueue/fsevent 直接绑定：重复造 notify 的轮子，否决。

## 后果

- watch 循环为 std mpsc + tokio 桥接（notify 是同步回调），单任务顺序应用——v1 不并行；
- journal 表新增（schema v3：只增，合规）。

## 重新评估条件

- notify 上游破坏性变更或停更 → 评估 debouncer 全家桶或自写平台层；
- 事件吞吐不足（>10k events/s 场景）→ 批量合并窗口调参或分片 watch。

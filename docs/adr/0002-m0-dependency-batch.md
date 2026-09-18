# ADR-0002: M0 元数据批次依赖集（sqlx/tokio/axum/blake3/walkdir/serde）

状态: 已接受 · 日期: 2026-09-18 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M0-WP02、调研方案 §6 选型清单、执行方案铁律 8

## 背景

M0-WP02 引入真实存储（SQLite）与可体验的演示面，首次需要产品级依赖。铁律 8 要求
新增顶层依赖先 ADR。本批依赖全部出自已批准的调研方案 §6 选型清单，此处集中定案。

## 决策（6 项，版本 2026-09-18 crates.io 核验）

| 依赖 | 版本 | 用途 | 理由 |
|---|---|---|---|
| sqlx | 0.9 | 元数据引擎 L0（SQLite，编译期校验 SQL） | 调研方案既定选型；Spacedrive V1 教训的「无聊地基」 |
| tokio | 1.53 | 异步运行时 | 生态事实标准，sqlx/axum 前提 |
| axum | 0.8.9 | 演示面 HTTP 服务（M1 网关同栈预演） | tokio 生态官方 web 框架 |
| blake3 | 1.8.7 | 内容寻址（ContentIdentity 主键） | 调研方案 §5.9：SIMD 多线程，与 iroh-blobs 同构 |
| walkdir | 2.5 | 目录遍历（扫描器骨架） | 无聊稳定；自写遍历的边角（符号链接/循环）不值当 |
| serde + serde_json | 1.0 | 演示面 JSON API | web 面事实标准 |

## 备选

- rusqlite（同步）替代 sqlx：更轻，但放弃编译期 SQL 校验且与 M2+ 的 async 栈割裂，否决；
- 自写 JSON 序列化：不严肃，否决；
- 手写目录遍历：节省一个依赖但引入边界缺陷面，否决。

## 依赖方向裁定

`partisync-graph → partisync-cas`（同层依赖）：ContentIdentity 需要内容哈希（blake3），
哈希归属 CAS。同层横向依赖在此处批准一次（理由：内容身份是图谱与仓库的天然接缝），
其余同层横向依赖仍需单独 ADR。

## 后果

- 编译时间显著增加（sqlx+axum+tokio 全家桶，首次全量构建约数分钟）——接受；
- deny.toml 许可证面：上述依赖均为 Apache-2.0/MIT，白名单已覆盖（cargo deny 首跑验证）；
- Cargo.lock 入库，`--locked` 可复现。

## 重新评估条件

- sqlx 0.x API 漂移造成季度级维护负担 → 评估 rusqlite + 手写仓储；
- axum 0.9 breaking → 网关 M1 前统一升级演练。

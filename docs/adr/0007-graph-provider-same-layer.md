# ADR-0007: graph → provider 同层依赖（远端索引缝合点）

状态: 已接受 · 日期: 2026-09-19 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: ADR-0002（graph→cas 先例）、SPEC M1-WP00、AGENTS crate 地图

## 背景
M1-WP02 消费面：远端存储（S3/WebDAV）目录要入图谱，索引逻辑需要同时触达
Provider（读远端）与 Store（写图谱）。两者同为领域层。

## 决策
`partisync-graph → partisync-provider` 同层依赖，理由同 ADR-0002：
「远端索引」是图谱与存储抽象的天然接缝。远端索引实现置于
`partisync-graph::remote_index`（与本地 indexer 并列，共享 entry/closure 语义）。

## 备选
- 索引逻辑放 CLI（壳层组合两依赖）：索引器含测试与复用面（watch/jobs 同源语义），放壳层不可测；否决。
- provider 反向依赖 graph：错误方向，否决。

## 后果
- 领域层横向依赖第 2 处（cas、provider），均以 ADR 逐例批准；
- AGENTS crate 地图注记更新。

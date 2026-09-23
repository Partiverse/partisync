# M5-WP02 基准与验收报告 —— 联邦路由协议（Multi-Hub Federation）

任务: M5-WP02-T07 · 日期: 2026-09-24 · 环境: darwin arm64（Apple Clang，
loopback，单进程双 hub；注册表组 = 单节点 raft，选举 300-600ms/心跳 50ms）·
规格: [specs/M5-WP02.md](../../specs/M5-WP02.md)

## 1. 交付总览

| 任务 | 主题 | Commit |
|---|---|---|
| T01 | 规格与任务卡落档 | `fa283b6` |
| T02 | 路由视图 raft 化（`RegistryCmd::SetRoute`/`r-route`/`merge_route`） | `5a262de` |
| T03 | 联邦线协议（`FedFrame`/`FedClient`，复用 net.rs 帧原语，零新依赖） | `83e9dfd` |
| T04 | 联邦服务端与反熵视图同步（`FedHandle`，可克隆句柄） | `cf5a284` |
| T05 | RouteClaim 路由协商与 `resolve_space` | `daae767` |
| T06 | 设备面 redirect 门（`HubService::route_for`） | `3ca0109` |
| T07 | 本报告 + 集成矩阵收官 | 本提交 |

## 2. 基准结果（`t07_bench_route_query_and_view_exchange`，n=2000）

| 指标 | 实测 | 口径 |
|---|---|---|
| **RouteQuery P50** | **146.5 µs** | 长连接往返（含 ReadIndex 线性一致确认），10⁴ 行视图中按键查询 |
| **RouteQuery P99** | **297.9 µs** | 同上 |
| **全量握手（10⁴ 行）** | **133.4 s**（≈75 行/s 吸收） | HelloAck 往返（~0.9 MB JSON 传输可忽略）+ 吸收端逐行 `SetRoute` raft 提交 |

**结论**：
- 路由查询远优于预期（P99 < 0.3 ms，规格 §风险「10⁴ 行 1MB 握手传输」的
  传输侧压力实测不存在——稳态反熵差量通常 ≤个位数行，5s 周期无压力）。
- **吸收端为已知瓶颈**：冷启动引导（cold join）10⁴ 空间需 ~2.2 min，来源是
  逐行单提交（~13 ms/commit，单节点组 fsync 主导）。**触发条件评估**：
  稳态运行（差量同步）不受影响；当单次吸收 >10³ 行（冷加入大联邦/批量
  转移）时，应立「`RegistryCmd::SetRoutes` 批量合入」后续卡（单日志条目
  批量 apply，预期 >100×）。v0.1 不做——语义正确性已由合并规则保证，
  性能优化独立成卡符合原子交付铁律。

## 3. 验收标准映射（SPEC §验收）

| 验收项 | 证据（`tests/m5_wp02.rs`） | 结果 |
|---|---|---|
| 路由视图 raft 化 + crash 不丢 | `t02_set_route_roundtrip_and_full_view`、`t02_routes_survive_crash_and_reopen` | ✅ |
| 合并确定性（epoch/hub_id/幂等） | `t02_merge_*`（5 测）+ `t03_absorb_routes_merges_and_is_idempotent` | ✅ |
| 双 hub 全链 claim→sync→query→redirect | `t07_full_chain_claim_sync_query_redirect` | ✅ |
| 并发 claim 收敛（hub_id 小者胜） | `t05_concurrent_claim_deterministic_lower_hub_id_wins`（含知情 epoch+1 = 归属转移） | ✅ |
| 协商败者即时改写（不等周期握手） | 同上（裁决 `winner` 吸收路径）+ `t05_claim_fresh_space_accepted_both_sides` | ✅ |
| 分区恢复（陈旧读 + 重连收敛） | `t04_peer_down_serves_stale_view_then_rejoin_converges`、`t05_peer_unreachable_resolve_miss_and_claim_is_local_proposal` | ✅ |
| redirect 门三分支 | `t06_route_for_three_branches` | ✅ |
| 多空间批量收敛矩阵 | `t07_multi_space_matrix_converges`（5 空间双向） | ✅ |
| 线协议变体守卫/往返 | `t03_client_roundtrip_all_variants`、`t03_variant_mismatch_is_protocol_error` | ✅ |
| 基准登记 | 本报告 §2 | ✅ |
| 回归（fmt/clippy/test 全绿；M3-WP01/02/03、M5-WP01 套件原样通过） | 本地门禁 + CI（`RegistryCmd` 新增变体 JSON 向后兼容） | ✅ |

测试合计：`m5_wp02` 21 通过（+1 ignored 基准）；WP01 直连存档、M3 三套件、
M5-WP01 套件原样全绿。

## 4. 与规格的偏差记录

1. **全链 redirect 断言走 FederationView**（`t07`）：`HubService::route_for`
   语义由 `t06` 独立覆盖；联邦面与 service 面共享 `RegistryService` 需
   `Arc` 化接线，归设备侧执行器接线卡（iroh 通道跟随重定向，M5-WP00 §1
   后续），本 WP 规格契约 4 未受影响。
2. **`HubService` 持有 runtime 的复用口径**：`route_for` 经自管 runtime
   `block_on`（与同文件方法一致）； FederationView 全异步形态避免
   `Handle::block_on` 进 tokio task（T02 实现注释）。

## 5. 后续卡建议（不阻塞收官）

- **联邦批量合入**（触发条件见 §2）：`SetRoutes` 单日志条目 + 吸收端批量。
- **iroh 传输替换联邦口**：version 字节留位已兑现（裁定 3）；统一设备/联邦
  加密传输，同时消除联邦口无认证风险（D2 OSCP 进场重审项）。
- **自动 failover**：owner 失联自动改判（需 hub 存活判据 + 租约语义，
  SPEC §风险明示 v0.1 不做）。

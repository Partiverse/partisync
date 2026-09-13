# ADR-002: 就绪/存活探针语义分离与消费场景

## Status
Accepted

## Date
2026-09-13

## Context

P2 安全加固批（A2-02）引入了两个探针，职责需要长期固定下来：

- `GET /healthz` — **liveness**：纯存活语义，恒 200（只要进程在跑）；
- `GET /readyz` — **readiness**：对 Postgres / Meilisearch / 存储根逐项探测，任一不可达 503，
  并按依赖列出 `ok` 二元状态。

P2 独立复审（`docs/evidence/audit-p2-independent-probe-timeout.md`）指出两点：

1. **F-I2（low）**：`/readyz` 在当前 docker-compose 部署中**没有消费者**——compose 不支持
   readiness 门控；server 容器的 healthcheck 正确地使用 `/healthz`（存活语义）。PG/Meili
   中途故障时 `/readyz` 会 503，但没有编排器据此摘流量。
2. **F-I7（info）**：`/readyz` 的 `error` 字段曾回显驱动错误串，泄露内部拓扑
   （DSN 主机、容器名、DNS 地址）；P2.1 批已改为二元口径（`unreachable`/`unavailable`），
   具体错误仅进服务端日志。

## Decision

1. **保留 `/readyz` 作为一等公民端点**，不为「compose 不消费」而删除或降级：
   - 它是 K8s `readinessProbe`、未来 Swarm/Nomad/反向代理健康门控（如 Traefik/Nginx
     `max_fails` + 主动检查）的直接接入点；
   - 它是 L2 验证脚本（`scripts/l2-p2-hardening.sh` S6）做依赖故障注入断言的观测面。
2. **明确非目标**：MCD/阶段二 compose 单机部署**不引入**服务网格或外部 readiness 编排；
   PG/Meili 故障期间 server 进程保持运行（liveness 不受影响），重启交由 `restart: unless-stopped`
   与进程自身崩溃恢复处理。
3. **错误信息口径**：探针响应对外一律二元（`ok` + 固定文案），拓扑细节只进服务端日志。
   该原则适用于未来新增的所有依赖探针。
4. **探测实现口径**：依赖探针必须是「真实操作」而非 TCP 连通性——PG 走真实 SQL
   （`CountAssets`）、Meili 走 `/health`、存储走真实写探针（临时文件 create+delete，ADR 见
   `internal/storage.Healthy`）。禁止退化为 `os.Stat` 式假探针。

## Alternatives Considered

- **删除 `/readyz`**：简化表面，但放弃了 K8s 迁移路径与故障注入观测面；否决。
- **compose 引入自定义健康门控（如 healthcheck 依赖链）**：compose 不支持 readiness 编排，
  自制脚本增加运维面而收益为零；否决，留待 K8s 化时一并解决。
- **`/readyz` 保留详细错误串**：对内排障更方便，但探针端点常暴露在代理可达路径上；
  以「日志排障」替代「响应排障」，排障成本增加一次 `docker logs`，值得。

## Consequences

- 阶段二若上 K8s，readiness 直接指向 `/readyz`，无需改动服务端。
- 依赖故障的排障入口是 `docker logs partisync-server`（搜 `readyz:` 前缀）。
- 新增依赖（如阶段二的连接器、对象存储）时，须同时给 `/readyz` 增加真实操作探针项。

# M8-WP03-T03 语义 spike：多租户过滤 × 联邦路由（任务卡 R4 处置）

> 任务: M8-WP03-T03 · 日期: 2026-10-02 · 性质: 语义 spike 报告
> （任务卡约定：本报告入仓后 router/registry 过滤改动方可合入）·
> 输入: SPEC M8-WP03 §2.3、M7-WP03-T03 §3 候选、M5-WP02 联邦路由契约
> 负责人: @lead · 执行: GLM-5.3-Flash (ZCode)

## 1. 现状核实（HEAD `cee00eb`）

- **空间路由门** = `HubService::route_for(space_id, self_hub_id)`
  （service.rs）：registry `routes` keyspace 查询 → Local / Redirect /
  Unknown 三分类（M5-WP02 T06 契约 4）；
- **列举面** = `RegistryService::routes()`：全量返回路由行（无任何
  可见性过滤）；
- **联邦面** = `federation::absorb_routes`：联邦视图合并进本地 routes
  （M5-WP02 判例口径）；
- **租户概念现状**：无（SpaceRow 含成员表/盐，无归属域）；
- 结论：**租户过滤的注入点是 registry 读取面（route/routes），不是
  router.rs**（后者是 sharded 元数据分片路由，与空间可见性无关）——
  任务卡 R4 的「与联邦路由谓词组合」实际落点为 `route_for` +
  `absorb_routes` 两处。

## 2. 语义裁决（实施契约，批准即冻结）

### F1 `route_for` 租户过滤

```
route_for(space_id, self_hub_id, viewer_tenant: Option<&str>) -> RouteDecision
```

| space 归属 | 行为 |
|---|---|
| 未归属任何租户（公共/未分组） | 维持三分类（Local/Redirect/Unknown） |
| 归属 viewer 同租户 | 维持三分类 |
| 归属**他租户** | **Unknown**（不可见） |

裁决理由：返回 Redirect 会泄露「该 space 存在且在别处」（租户存在性
泄露）；Unknown 与「不存在的空间」不可区分，符合不可见性目标。

### F2 `routes` 列举过滤

```
routes_tenant(viewer_tenant: Option<&str>) -> Vec<RouteRow>
```

返回 = 未归属行 ∪ viewer 租户行；他租户行剔除。原 `routes()`
保留为**管理员全量视图**（doc 注明），门面新方法不动旧签名。

### F3 `absorb_routes` 联邦合并过滤

联邦视图吸收**按本地租户域过滤**：absorb 前剔除「本地已归属他租户」
的 space 行（防止联邦侧路由行把不可见空间的持有 hub 地址带进本地视图）；
viewer 过滤仍由 F1/F2 在读取面执行（写入面与读取面双闸）。

### F4 认证面缺口（诚实边界，登记不扩）

v0.1 `viewer_tenant` 为**调用方声明**（无认证绑定）——「不可见」是
策略面语义，非安全边界；真正的租户认证（token → tenant）随 M9 SSO
（SPEC 非目标表）。审计面：tenant 上下文随 actor/object 记录（现有
audit 行格式够用，`tenant:{id}` 计入 object 前缀）。

## 3. 数据面设计

- `r-tenant` keyspace：`space_id → tenant_id`（一行一空间）——
  **反向表**（空间→租户）而非正向（租户→[spaces]）：可见性判定是
  「给 space 查租户」，反向表 O(1) 免全扫；正向列举由全扫聚合（量小）；
- RegistryCmd 新增：`AssignTenant { actor, space_id, tenant_id }` /
  `UnassignTenant { actor, space_id }`——**Assign 时若 space 已属他
  租户 → 拒绝位 `FLAG_FORBIDDEN`**（一空间至多一租户；换绑 = 先
  Unassign 再 Assign 的显式两步，无静默改属）；
- 不做：租户实体的独立 CRUD（tenant_id 是不透明字符串，创建即用）、
  跨租户配额聚合、租户级 audit 分视图。

## 4. 探针计划（实施 PR 验收）

1. Assign/Unassign CRUD + 一空间一租户约束 + 换绑拒绝路径；
2. F1：他租户 space route_for → Unknown（三态不泄露）；
3. F2：routes_tenant 过滤（未归属/同租户/他租户三域）；
4. F3：absorb 过滤（他租户行的联邦地址不入本地视图）；
5. 审计：Assign/Unassign 入审计链（`tenant_assign` 动作）。

## 5. 复核日志

- 现状核实基于 HEAD `cee00eb` 源码阅读（router.rs 与 service.rs 职责
  边界澄清为本 spike 主结论之一）；裁决处均有理由链，无凭空断言。

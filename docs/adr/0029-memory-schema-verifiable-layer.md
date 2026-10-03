# ADR-0029: 可验证记忆层 schema 演进——graph 新实体域 + 独立二叉证明树

状态: 草稿 · 日期: 2026-10-03 · 决策人: @lead（草稿随 SPEC M9-WP02 批准生效）
关联: SPEC M9-WP02 · docs/reviews/M9-WP02-memory-competitor-scan.md ·
docs/reviews/M9-roadmap-proposal.md §5-WP02 · ADR-0026（FUSE 网关语义，oplog 沿用）

## 背景

M9-WP02 需要给 graph store 增加第三种同步实体——记忆资产（memory）：AI
agent 与用户写入的事实条目，要求 (a) 经既有 oplog/bisync 管线多端同步，
(b) 具备防篡改可验证性（Merkle inclusion proof，竞品摸底证实 mem0/Letta
均无此能力，是产品差异化主轴）。graph store 现状（M9-WP01 后）：SQLite
单库，实体域 entry/tag，oplog 以 (entity, op) 字符串对分发应用
（session.rs `apply_remote` match 臂）；Merkle 现状：`merkle.rs` 为对账用
分桶哈希树（P7），单层 bucket_hash，**无 inclusion proof API**。

两个决策点：memory 放哪（schema 演进路径）；证明树怎么建（树形选型）。
约束：铁律 8 零新依赖倾向；P6（同步收敛）/P7（对账树）既有不变量不得
回退；一期单人带宽须收窄（M9-WP00 §5 风险登记）。

## 决策

**memory 为 graph store 新实体域：新增独立 `memory` 表（additive-only
幂等迁移，沿现行 `include_str!` 机制），oplog 走 `entity="memory"` 新
match 臂；证明树为独立 RFC 6962 式二叉 Merkle（blake3，叶/节点域分离，
叶按键升序，审计路径 O(log n)），与对账分桶树并存、互不复用。**

配套契约（细节归 SPEC §2）：

- `memory_id = blake3("M\x1f" + content + tags canonical + metadata
  canonical)`，内容寻址；同 (content, tags, metadata) 二次写幂等。
- 行簿记字段（created_ns/origin_device/hlc）进叶哈希（沿 entry_leaf
  「含全部可比字段」判例），双端同 id 独立写由 HLC LWW 收敛一行。
- `memory_root` 为**派生值**：可随时从 memory 表全量重算；持久根仅作
  快照缓存，重算不一致 = 检出篡改/损坏（verify 语义依据）。

## 备选方案

**B1：复用 entry 域（EntryKind 增 Memory=2）**——否决：entry 域主键是
path、带 entry_closure 父目录链与 FUSE/CLI 直写语义，memory 无路径语义；
且 entry 叶集是 P7/P19 树根的输入，混入 memory 叶会变更既有根口径，
wiring_e2e 同根判据与全部对账探针受牵连。

**B2：独立 memory store（新 sqlite 文件 / 新 crate）**——否决：失去
oplog/bisync 免费复用（多端同步是记忆层核心卖点）；桌面 McpSidecar 单
`--db` 假设被打破（M6-WP03-T05 契约）；两库事务边界（写 memory + 写
oplog 原子性）无收益只有成本。

**B3：扩展分桶树做 proof（桶路径作凭证）**——否决：凭证尺寸 O(桶内
叶数) 而非 O(log n)；验证方须复刻 partisync 分桶算法，第三方独立可验证性
（生态互操作的核心卖点）不成立；且分桶树语义是对账下钻（P7），混入承诺
语义会让两套不变量互相捆绑。

**B4：MPT/Patricia 树**——否决：一期叶量 10⁴–10⁵，二叉排序树足够；
MPT 的键序无关性与增量更新能力一期用不上（YAGNI，全量重算亚秒级沿
merkle.rs 既有注释口径）。

## 后果

正面：同步零新机制（oplog/bisync/P6 直接覆盖 memory 域）；证明标准
（RFC 6962 式审计路径）第三方可独立复算；对账树与证明树解耦，P7/P19
零回退；零新依赖（blake3/serde_json 既有）。

负面（放弃的东西）：两棵树并存带来双份叶编码维护义务（entry/tag/link
叶 vs memory 叶），叶编码变更须同时登记两处影响面；全量重算 O(n) 意味
着 memory_write 每次重算根在大叶量下有成本——一期以「写后重算、10⁵ 叶
亚秒级」为口径，增量树挂 KPI 不达标触发器；content-addressed id 使
簿记字段（时间/设备）不参与身份，同内容不同时刻双端写收敛为一行
（HLC LWW 裁决），放弃「同内容多版本并存」语义。

## 重新评估条件

- memory 叶量进入 10⁶ 量级或 write P95 超 100ms → 增量树（B4 重估）。
- MCP 生态出现事实标准的可验证记忆 schema（如 MCP 官方 memory 能力
  进 protocol spec）→ 对齐评估，工具面命名兼容优先。
- 记忆删除/更新/GC 需求落地（AI workflow 阶段）→ 本 ADR 增补修订登记
  （tombstone 叶口径须评估根稳定性）。
- entry 域需要同类防篡改承诺（对账树升级为承诺树）→ 评估两树合一。

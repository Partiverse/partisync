# 属性测试不变量清单（P-registry）

> 执行方案 §4.2 的落地登记册。每条不变量是「可执行的数学」：属性测试必须引用本清单编号，
> 新增不变量先登记于此再写测试（G1 门禁）。

| # | 模块 | 不变量 | 测试形态 | 生效阶段 |
|---|---|---|---|---|
| P1 | CDC 分块 | 同输入 → 逐位相同的分块序列（确定性） | proptest 任意字节流 | M0-WP03 |
| P2 | CDC 分块 | 在位置 p 插入字节后，p 之后一定距离外的块边界不受扰动（内容定义性） | proptest + 边界距离断言 | M0-WP03 |
| P3 | CDC 分块 | 块长 ∈ [min,max]，均值落在目标带宽内 | 统计断言 | M0-WP03 |
| P4 | CAS | hash(content) == root(chunks)；同内容二次入库引用计数 +1、不新增块 | 模型对照测试 | M0-WP03 |
| P5 | HLC/oplog | 时间戳严格单调；同物理时间并发可全序 | 并发线程模型测试 | M0-WP01（HLC）/ M2-WP01（oplog） |
| P6 | 同步收敛 | 任意两副本应用同一操作集（任意顺序、含重复）后状态等价 | 线性化模型测试 | M2-WP01 |
| P7 | Merkle 对账 | 无假阴性（树根相等 ⇒ 概率界内无漏检差异）；分歧区间必被下钻覆盖 | 随机差分注入 | M2-WP03 |
| P8 | 崩溃日志 | 任意 failpoint 崩溃 → 重放后一致且不丢已确认操作 | failpoint 枚举 | M0-WP05（本地）/ M2（全量） |
| P9 | Provider caps | 声明无某能力的后端绝不走该能力路径 | caps↔行为一致性测试 | M1-WP01 |
| P10 | 路径映射 | Unicode/非法字符在每 provider 编码方案下 round-trip 无损 | 表驱动 + 随机 Unicode | M1-WP01 |
| P11 | 冲突策略 | 同名双改 → 两者皆可寻址，血缘可查 | 模型测试 | M2-WP02 |
| P12 | 传输完整性 | 任意丢块/乱序/重传 → 重组后 BLAKE3 必验；验败必拒 | 注入测试 | M1-WP03 / M2-WP05 |
| P13 | 扩展沙箱 | 未授予能力（capability）的 wasm component 调用 FS/网络/时钟 API 必须失败（默认拒权；M7 实施全量 = **per-call 拒绝**：未注权 component 对宿主函数的每次调用必败，非仅实例化期），且失败信息不泄露宿主路径/环境变量 | spike 探针：缺权 component 实例化/调用必败 + 错误文本对宿主路径与 env 的否定断言（SPEC M6-WP04 §2.4）；M7-WP01-T03 实施探针：注权时钟调用可达（反向验证）+ 未注权 interface 不可解析 + 零注权拒绝不泄露（SPEC M7-WP01 §3）。**实施面以更强形式满足**（2026-09-29 登记）：Component Model 注权是实例化期语义——import 静态解析，宿主无对应 interface 实例时 component 无法实例化、guest 代码零执行，拒绝先于任何潜在调用，严格强于 per-call | M6-WP04（spike 探针）→ M7-WP01（实施全量） |
| P14 | 注权 manifest | capability manifest 校验失败（非法声明 / 缺失 manifest / 扩展工具与内建撞名）→ 加载即拒，拒绝先于任何宿主函数暴露；注入面 = 声明面 ∩ 宿主白名单，未声明的 capability 一律不注入 | 加载拒绝探针：manifest 篡改 / 缺失 / 撞名用例断言加载错误先于实例化（SPEC M7-WP01 §2.2 白名单冻结表） | M7-WP01 |
| P15 | FUSE 语义面 | mountpoint-s3 式「诚实非 POSIX」（SEMANTICS.md）：拒绝面操作（unlink/rmdir/rename/mkdir/mknod/symlink/setattr/已存在文件写打开）在**破坏性效果发生前**显式失败（EPERM/EACCES），拒绝后文件原样；新文件写为严格顺序追加（跳写 EINVAL），release 后不可再写 | 产品真挂载探针（环境门控；Linux 容器 `--device /dev/fuse --cap-add SYS_ADMIN` 实测路径）逐项断言 errno + 拒绝后读回一致（SPEC M8-WP01 §2.2/§3；crates/partisync-fuse/SEMANTICS.md）。**2026-09-30 随 M8-WP01-T01 由 M7-WP03 spike 候选转正**（实现 crates/partisync-fuse，探针 crates/partisync-fuse/tests/probe_mount.rs） | M7-WP03（spike）→ M8-WP01（实施） |
| P16 | FUSE 写回一致性 | 写回操作（unlink/rmdir/rename）**先日志后应用**：日志项未落盘不产生破坏性效果；apply 失败时 backing 保持原状；crash 后重放**幂等**（同一日志项重放 N 次结果一致），backing 不出现半提交状态 | proptest ≥1000 例：随机操作序列 append 全部未应用 → 重放两次快照逐字节一致且与模型终态一致（SPEC M8-WP07 §2.3/§3；crates/partisync-fuse/src/writeback.rs）。**2026-10-03 随 M8-WP07-T02 转正**（SPEC §7 落锤 Q2：WAL=backing 旁 JSONL） | M8-WP07（实施） |
| P17 | Hub 线性一致读 | 并发写序列下，任意客户端已观察到的写结果不被后续读回退（读单调）；follower 读经 ReadIndex 确认 commit index 后应答，不返回过期权威行 | proptest 并发写 + 交错读 ≥1000 例（模型对照，SPEC M8-WP04 §2.3/§3；P16 已被 M8-WP07 预订故跳号）。**2026-10-01 随 M8-WP04-T03b 实现**（crates/partisync-hub/tests/wp04_p17.rs：1000 例模型对照 proptest（单 Hub 复用 + case id 命名空间）+ raft 门面真双线程并发单调探针（写者顺序推进版本 / 读者 ensure_linearizable 后观察不回退）） | M8-WP04（实施） |
| P18 | Hub 审计链 | 审计日志任意行被篡改/删除/重排 → 链式哈希验算必报（逐行 hash 含前行摘要；导出文件附链头链尾 digest 可独立复算） | 篡改探针：改行/删行/交换序三类注入 + 链验失败断言（SPEC M8-WP03 §2.1/§3）。**2026-10-01 随 M8-WP03-T01 实现**（crates/partisync-hub/src/audit.rs 链式 blake3 + `r-audit` keyspace + 异步 sink 队满阻塞写；探针 crates/partisync-hub/tests/wp03_audit.rs：字段完备/顺序/篡改必报/导出链头链尾独立复算/去重命中不记审计。改行注入实测；删行/乱序由 seq 连续与 prev 衔接断言覆盖） | M8-WP03（实施） |
| P19 | fuse→sync 装配 | 挂载写事件经装配层进 graph/oplog：(a) 事件折叠与 CLI 直写在同库形态下叶口径一致（同参数 `add_entry` → 同 Merkle 根，wiring_e2e 判据生产化）且 bisync 不动点后双端 `pending_oplog` 清零（收敛；对端行属主=源设备为 M2 既有语义，根不对称不属本不变量）；(b) 本端 oplog 行不被对端回流应用（origin 剪枝，`skipped_self_origin` 计数增长）；(c) 同一事件重复投递 → graph 行幂等（entry 数稳定、终态一致） | e2e：channel 注入事件序列 + 折叠/直写同根断言 + bisync 不动点断言 + origin 剪枝计数断言 + 重复投递幂等断言（SPEC M9-WP01 §2.4/§3）。**2026-10-03 随 M9-WP01-T01 登记**（实现 crates/partisync-gateway/src/wiring.rs 装配层 + crates/partisync-fuse/src/events.rs 事件面；探针随 T02 `crates/partisync-gateway/tests/wp01_wiring.rs`） | M9-WP01 |
| P20 | 记忆层可验证承诺 | (a) **确定性**——证明树根是 memory 可见集的确定性函数：同集同根，与插入顺序/时刻无关（叶按 memory_id 升序，RFC 6962 MTH）；(b) **可靠性**——`verify_inclusion` 对属于承诺集的叶成立；篡改 leaf/audit_path/root 任一字节必败；审计路径长 ≤ ⌈log₂ n⌉+1；(c) **可重算**——`memory_root` 快照可由持久层全量重算重建；篡改行内容（content_hash 列级失配）或篡改快照根后 `verify_memory` 必报 `ok=false` 而非静默通过 | proptest/探针：乱序插入同集同根 + 全叶包含验证 + 三类单字节篡改必败 + 路径长对数界 + 直接 SQL 改行/改根后 verify 失败断言（SPEC M9-WP02 §2.7/§3）。**2026-10-04 随 M9-WP02-T02 登记**（实现 crates/partisync-graph/src/memory.rs + store.rs memory_write/verify_memory；探针 `crates/partisync-graph/tests/wp02_memory.rs`） | M9-WP02 |
| P21 | 扩展签名装载 | 扩展装载必经验签：(a) 无 `.minisig` 或签名/格式非法 → 装载期拒绝（`Unsigned`/`BadSignature`），拒绝先于 component 字节进入 wasmtime 编译器；(b) 篡改 `.wasm` 任一字节或未知钥签名必拒；(c) 内嵌锚定公钥（发布双钥）任一通过即放行，无 unsigned 豁免通道 | 五路探针：合法签名装载通过 + 单字节篡改必拒 + 缺签必拒 + 未知钥必拒 + 拒绝变体断言（验签向非编译向）（SPEC M9-WP04 §2.2/§4）。**2026-10-04 随 M9-WP04-T02 转正**（实现 crates/partisync-ext-host/src/signature.rs + registry.rs 装载序插桩；探针 crates/partisync-ext-host/tests/wp04_signature.rs 五路 + scan 孤儿签名附加 6/6 绿，PR 记录随合入回填） | M9-WP04 |

## 本地类型级不变量（不属于 P 序列，随 crate 登记）

| ID | 模块 | 不变量 | 生效 |
|---|---|---|---|
| L1 | core::ulid | encode(parse(x)) == x（26 字符 Crockford，大小写不敏感解析） | M-1-WP07 |
| L2 | core::ulid | ts1 < ts2 ⇒ ulid(ts1,·) < ulid(ts2,·)（字节序=字典序=时间序） | M-1-WP07 |
| L3 | core::ulid | Display 恒 26 字符且 ∈ Crockford 字母表；timestamp_ms/random_part 提取往返一致 | M-1-WP07 |
| L4 | graph::store | entry_closure 子树查询 ≡ 朴素递归遍历（任意随机树，模型对照） | M0-WP02 |
| L5 | graph::jobs | 「中断→resume」最终 stats ≡ 「全量直index」（恢复等价性） | M0-WP05 |

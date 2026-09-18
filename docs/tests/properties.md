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

## 本地类型级不变量（不属于 P 序列，随 crate 登记）

| ID | 模块 | 不变量 | 生效 |
|---|---|---|---|
| L1 | core::ulid | encode(parse(x)) == x（26 字符 Crockford，大小写不敏感解析） | M-1-WP07 |
| L2 | core::ulid | ts1 < ts2 ⇒ ulid(ts1,·) < ulid(ts2,·)（字节序=字典序=时间序） | M-1-WP07 |
| L3 | core::ulid | Display 恒 26 字符且 ∈ Crockford 字母表；timestamp_ms/random_part 提取往返一致 | M-1-WP07 |
| L4 | graph::store | entry_closure 子树查询 ≡ 朴素递归遍历（任意随机树，模型对照） | M0-WP02 |

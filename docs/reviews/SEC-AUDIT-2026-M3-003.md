# M3 补充审计报告——会话 sess_4323309e 工作清点与闭环建议

> **报告编号**: SEC-AUDIT-2026-M3-003  
> **审计范围**: 接续 SEC-AUDIT-2026-M3-001(治理面)与 SEC-AUDIT-2026-M3-002(密码学面)的**第三方补充审计**——聚焦"未覆盖"清单的工作树状态核查、跨规格一致性、独立事实再核验  
> **审计日期**: 2026-09-20  
> **审计主体**: 独立 AI 审计员(无起草/治理会话上下文;按"sess_4323309e 补充完整审计工作"指令执行)  
> **审计基准**: AGENTS.md 十条铁律、SEC-AUDIT-2026-M3-001/002、M2 checklist、SEC-AUDIT-2026-M2-001、ADR-0000/0002/0010/0011、RFC/OWASP/ crates.io 实时事实  
> **审计结论**: **Mixed(部分通过,部分阻断)**——M3 规格/ADR 治理面与密码学面整改均成立,但工作树状态与多份文档/构建事实与断言存在出入,需在签核前关闭

---

## 一、执行摘要

| 维度 | SEC-AUDIT-2026-M3-001 结论 | SEC-AUDIT-2026-M3-002 结论 | **本审计(SEC-AUDIT-2026-M3-003)** |
|---|---|---|---|
| M3 规格/ADR 治理面 | Conditional Pass(P1/P2 已闭环) | — | **降级:发现 P0 工作树事故** |
| 密码学面 | — | Pass with Caveats | **维持(SEC-AUDIT-2026-M3-002 独立交叉确认)** |
| 跨规格一致性 | 未覆盖 | — | **新发现:P7 不变量交接存在但完整证明推迟到 WP03** |
| 独立事实核验 | 部分(sled 时间线未核) | — | **新发现:sled 1.0.0-alpha.124 已发布,ADR-0011 否决理由部分失真** |
| 构建/测试门禁 | 治理登记闭环,未实跑 | 跑了 sync crate | **新发现:hub crate 工作树事故——tests 不编、clippy 不绿、未提交** |
| 依赖治理门禁 | 登记 deny 既有失败"另开任务" | licenses/bans 已 FAIL | **新发现:deny licenses/bans 当前实际仍 FAIL** |

**总评**:
- 文档治理面与密码学面**理论结论成立**;
- 但**仓库当前工作树状态**与文档声明的"签核→开工 T03→完成"叙事**不一致**——T03 实现处于"lib 编译过 / tests 失败 / clippy 失败 / 全部未提交"的状态,文档已登记的"P1/P2 闭环"叙事未覆盖此层。
- 在用户拍板"是否签核 ADR-0011 启动 WP01-T02→T03"前,**应先关闭工作树事故**,否则 WP01-T03 任务的门禁证据链断裂。

---

## 二、SEC-AUDIT-2026-M3-001"未覆盖"清单逐项核验

### 2.1 未运行 cargo check/build/test(hub 无代码可编)

**本审计结论:推翻此条**——hub 当前**已有未提交代码**(见 §四)。

证据:
```
$ git status --short
 M Cargo.lock
 M crates/partisync-hub/Cargo.toml
 M crates/partisync-hub/src/lib.rs
?? crates/partisync-hub/src/encode.rs       (8237 B)
?? crates/partisync-hub/src/entry_plane.rs  (6995 B)
?? crates/partisync-hub/src/shard.rs        (1437 B)
?? crates/partisync-hub/tests/              (含 wp01.rs)
?? docs/reviews/SEC-AUDIT-2026-M3-002.md
```

### 2.2 未审 WP02–WP06 规格

本审计范围仍限于 WP00/WP01/ADR-0011。WP02 规格确认**尚未起草**(`ls docs/specs/M3-*.md` 仅 WP00/WP01)。

### 2.3 未比对 M2-WP03 Merkle 对账与 WP01 §5 的 P7 不变量守护

**本审计:已比对,见 §三**——结论:P7 交接点存在,完整证明推迟到 WP03。

### 2.4 未实测 proptest 均匀性、分裂代价、读时修复放大率

**本审计:静态可论证读时修复的 N 上限保护(修订后 WP01 §4)**

`docs/specs/M3-WP01.md:90-93`(已合入修订):
> **读时修复**:`get_entry`/`list_children` 检测权威行与投影不一致(含投影缺失)时以权威行为准补齐投影(写回),返回值永远以权威行构造;**单次修复补写上限 256 行,超出转后台批量**——防大目录读放大击穿 300B 写放大口径(T06 实测)。

审计判断:
- 256 行上限的设定依据**未在 SPEC 中显式给出**(为何不是 128/512?),需要 WP01-T05 落地时配 proptest 反推;
- T06 验收仍要求"盘上字节/条目 ≤300B"作为硬门禁,256 上限是可被实测验证的;
- **修订方向合理,但具体 N 值需要 T05 落地时论证**。

### 2.5 未独立查证 sled 维护停滞时间线

**本审计:已独立核验,见 §五**——发现 ADR-0011 否决理由部分失真。

### 2.6 未确认外部审计公司候选名单

**本审计:已不适用**——D1 清偿口径在 SEC-AUDIT-2026-M3-001 §四与 M2-report §5/D1 已修订为"AI 独立密码学审计替代",由 SEC-AUDIT-2026-M3-002 实质完成。

---

## 三、M2-WP03 ↔ M3-WP01 §5 P7 不变量交接分析

### 3.1 M2-WP03 P7 设计(`docs/specs/M2-WP03.md:64-94`)

- **单 Store + 单 Merkle 树**(4/8/12-bit 分级桶)对账;
- 健全性条件:`∀d ∈ origins(a) ∪ origins(b): Seen_a(d) == Seen_b(d)`(全 origin 覆盖相等才能走快路径);
- 慢路径:根不等 → 下钻分歧区间 → 叶级 diff 修复;
- "无假阴性":blake3-256 碰撞概率 ~2^-128(可忽略),且根对任何单叶差异敏感。

### 3.2 M3-WP01 对 P7 设计的拓扑扰动

| M2 设计 | M3-WP01 改动 | 对 P7 的潜在影响 |
|---|---|---|
| 单 Store 单树 | entry 平面拆 256 哈希分片 + children 平面拆动态 range 分区 | 树根需跨 256+ 分区聚合;**聚合策略未定**(每分片子树 vs 整盘单树)|
| 单写者串行(M2-WP01 oplog) | 单写者串行保持(M3-WP01 §4) | 不变 |
| 投影 = 入口读写同一事务 | **跨平面无事务**(M3-WP01 §裁定 3),读时修复兜底 | **新风险面**:Merkle 行走时若同时跨越 entry 与 children 平面,两边投影陈旧度不同,根等值判断的语义是"双方看到相同的陈旧",还是"双方的真值相同"? |
| 增量树(YAGNI) | 不变 | 不变 |

### 3.3 交接点确认

- **M3-WP00 §工作包图**:`WP03 hub 对账服务:设备↔hub Merkle 对账(复用 M2 WP03 分级树,P7 无假阴性不变量跨端保持)`——明确"跨端保持"是 WP03 的任务。
- **M3-WP01 §5 一致性**:投影陈旧窗口由"WP03 对账收口"——明确责任移交 WP03。
- **M3-WP01 §风险 第 3 条**:WP02 复制层语义文档须重审此裁定(双人全审钉子)——明确责任移交 WP02。

### 3.4 审计结论:P1 高

- **交接点本身合理**,P7 守护的完整证明被显式延后到 WP03 规格;
- **风险**:WP03 规格在起草时如未充分考虑 ① 跨平面陈旧窗口 ② 256+range 分片聚合 ③ range 分裂协议中途的"双读视图"三要素,可能制造亚假阴性(双方根等但子树陈旧窗口下被错认为已收敛);
- **建议**:
  1. M3-WP03 规格 G0 批准前必须显式回答三要素:
     - Merkle 树根如何聚合(单棵跨平面虚拟根 vs 每分片子树 + 跨分片元根);
     - 跨平面陈旧窗口是否纳入"可观察状态";
     - range 分裂协议"双读视图"期间,Merkle 行走策略是否包含源+目标合并视图(类 WP01 §3 分裂协议);
  2. WP03 规格起草应引用本审计 §3.2 表作为已知风险面;
  3. 若 WP03 规格未起草,M3-WP01 不应签核为"最终可执行"——目前 WP01 §5 写入"WP02 文档展开并双人全审"的移交即可,但 §验收 第 6 项"崩溃一致性"的"splitting 态按协议收尾"应与未来 WP03 协议保持一致(目前未交叉引用)。

---

## 四、hub crate 工作树状态(关键阻断发现)

### 4.1 实际状态

`git status --short` 揭示:
- **3 个 .rs 源文件 + tests/ 目录 + dev-deps 段** 全部未提交(??)
- `lib.rs` 已修改未提交(M)
- `Cargo.toml` 已修改未提交(M,新增 blake3 依赖 + dev-deps)
- `Cargo.lock` 已修改未提交(M)

文件时间戳 `2026-09-20 02:11~02:13`(今日凌晨),且无对应 commit message 的 trailer。

### 4.2 编译/测试/clippy 现状

| 命令 | 结果 | 备注 |
|---|---|---|
| `cargo check -p partisync-hub`(仅 lib) | ✅ 通过 | lib crate 编译 |
| `cargo check -p partisync-hub --all-targets` | ❌ 7 errors | `tests/wp01.rs` 7 处 `prop_assert_eq!` 宏未导入(`use proptest::prop_assert_eq;` 缺失) |
| `cargo test -p partisync-hub` | ❌ 无法执行 | tests 不编 |
| `cargo clippy -p partisync-hub --all-targets -- -D warnings` | ❌ 1 error | `crates/partisync-hub/src/encode.rs:5` clippy::doc_lazy_continuation(注释缩进) |
| `cargo check --workspace --all-targets` | ❌ fails | 受 hub tests 影响 |

### 4.3 与 SEC-AUDIT-2026-M3-001 登记的不一致

SEC-AUDIT-2026-M3-001 §二 结论:"**本次审计验收:通过(Conditional Pass 的条件项即上表 5 项,全部闭环)**"。

但本审计揭示:
1. 闭环条目"P1-1 默认改 4M 行 + 单 Database + N keyspace + blake3::hash 钉死 + ADR 状态行"——全部在 SPEC/ADR **文档**层面闭环;
2. 但 T03 实现任务的代码层门禁**未通过**;
3. 工作树存在大量**未提交代码**,且**未提交即未走 PR 评审**,更未走双人终审;
4. SEC-AUDIT-2026-M3-001 §三 时序披露已诚实指出"签核决策独立作出,审计建议的『补正后再签核』未能先行"——但**未揭示工作树状态**。

### 4.4 审计结论:P0 阻断

**在关闭以下工作树事故前,WP01-T03 不应进入"完成"状态,M3-WP01 不应再次签核**:
1. `tests/wp01.rs` 修复 7 处 `prop_assert_eq!` 宏导入;
2. `src/encode.rs:5` clippy 注释缩进修复(或在该行加 `#[allow(clippy::doc_lazy_continuation)]`);
3. 把当前工作树(T03 实现 + Cargo.toml/lock 改动)整理为一条 commit(单 PR ≤400 行 diff 纪律照旧);
4. commit message 完整列出 trailer:`Task-ID: M3-WP01-T03`、`Spec: docs/specs/M3-WP01.md`、`ADR: docs/adr/0011-fjall-metadata-lsm.md`、`AI-Assist: <agent/model>`;
5. 提交后跑 `cargo test -p partisync-hub` 与 `cargo clippy -p partisync-hub --all-targets -- -D warnings` 全绿,作为验收证据。

**额外发现**:`M3-WP01-T02` 提交 `43d0c22` 标题宣称"ADR-0011 用户签核为已接受 + deny 白名单补 0BSD/BSL-1.0"——但 cargo deny 当前 `licenses FAILED + bans FAILED`(见 §六),**提交 message 与现实不符**,需要在 deny 整改完成后另开任务补正叙事。

---

## 五、独立事实再核验:ADR-0011 否决 sled 的依据

### 5.1 ADR-0011 原话(`0011-fjall-metadata-lsm.md:39`)

> **sled**:维护长期停滞(0.x 未达 1.0),不满足「无聊依赖」纪律——否决;

### 5.2 本审计独立核验(`cargo info sled`,2026-09-20)

```
sled = "1.0.0-alpha.124"
homepage: https://github.com/spacejam/sled
repository: https://github.com/spacejam/sled
crates.io: https://www.crates.io/crates/sled/1.0.0-alpha.124
```

### 5.3 审计结论:P2 中

- **事实陈述部分失真**:sled 已发布 `1.0.0-alpha.124`(2026-09-20 时刻),并非"0.x 未达 1.0"——ADR 起草时(2026-09-20 早些时候)可能为真,本审计时已不真;
- **否决结论仍站得住**:alpha 仍属"1.0 前快速演进带",且仓库活跃度、API 稳定性、生产可用性证据均弱于 fjall;sled 即便达到 1.0,本次选型仍倾向于 fjall;
- **建议**:ADR-0011 §备选 sled 段加一句"截至 2026-09-20,sled 已发布 1.0.0-alpha.124,维护已重启,但 v1.0 稳定时间表未定,本 ADR 不重启选型",把决策时点的实时事实写明,避免后人翻账;
- 此项**不阻塞签核**,但应在下次 ADR 修订时补正。

---

## 六、依赖治理门禁现状(接续 SEC-AUDIT-2026-M3-002 §六)

| 检查 | 结果 | 备注 |
|---|---|---|
| `cargo audit --no-fetch` | ✅ 0 vulnerabilities / 406 deps | 与 M2 报告一致 |
| `cargo deny check licenses` | ❌ FAILED | `deny.toml:11` "BSD-2-Clause" 白名单登记但实际未被命中;另 CDLA-Permissive-2.0 缺白名单(SEC-AUDIT-2026-M3-002 §六 已揭示)|
| `cargo deny check bans` | ❌ FAILED | 多版本 wildcard(M2 既有问题,非密码学)|
| `cargo deny check advisories` | ✅ ok | 干净 |
| `cargo deny check sources` | ✅ ok | 干净 |

**SEC-AUDIT-2026-M3-001 §二 非阻塞发现**与 **M3-WP01-T02 commit `43d0c22` message** 均称"deny 既有失败登记另开任务"——但**任务未起草、未指派、未排期**,违反 §一 验收结论"全部闭环"的字面意义。

**审计结论:P1 高(治理层)**
- deny 失败必须先解决再推进 WP01-T03+,否则 M3-WP01-T03 提交本身可能命中 bans 失败(M2-WP01-T03 等先例表明 deny 是 commit message 中明示门禁);
- 建议:在 WP01-T03 任务卡前**先**单开 `M3-WP01-T01a`(或类似)处理 deny 失败,与 WP01 任务卡解耦,避免依赖治理债务堆积。

---

## 七、本次审计独立新发现清单

| # | 级别 | 发现 | 证据 |
|---|---|---|---|
| 7.1 | **P0** | hub crate 工作树事故(未提交 + tests 不编 + clippy error)| §四 |
| 7.2 | P1 | deny licenses/bans 当前实际仍 FAILED,M3-WP01-T02 提交 message 与现实不符 | §六 |
| 7.3 | P1 | M2-WP03 ↔ M3-WP01 §5 P7 交接完整证明推迟到 WP03 规格,但 WP03 起草时需明确三要素(聚合/陈旧/分裂双读)| §三 |
| 7.4 | P2 | ADR-0011 否决 sled 理由部分失真(已发 1.0.0-alpha.124)| §五 |
| 7.5 | P2 | M3-WP01 §4 读时修复 256 行上限的依据未在 SPEC 中显式说明(为何不是 128/512?)| §二.2.4 |
| 7.6 | P3 | `partisync-hub/Cargo.toml` dev-deps `partisync-core = { path = "../partisync-core" }` 与 §非阻塞 中提及的"模块草图扩展点"未在 SPEC 涉及文件清单中体现 | crates/partisync-hub/Cargo.toml |
| 7.7 | P3 | cargo deny output 与 cargo-audit 数据不一致(M2 报告"1251 advisories",本审计"406 deps"——基数差异需核对 cargo-audit 与 deny 的索引)| §六 |

---

## 八、SEC-AUDIT-2026-M3-002 关键结论交叉确认

为避免"两份独立审计彼此矛盾",本审计对 SEC-AUDIT-2026-M3-002 的关键结论做了**逆向交叉抽查**:

| SEC-AUDIT-2026-M3-002 结论 | 本审计交叉确认 | 状态 |
|---|---|---|
| 4 项 M2 整改均实质完成 | 与 SEC-AUDIT-2026-M3-001 修订一致;`grep hkdf_blake3 crates/` 应仅命中报告自身(本审计未跑,但 M2-WP07 提交链 `4d60e29` 关闭此函数)| 维持 |
| `pairing_session.shared_secret` 仍明文落盘(store.rs:1842)| SEC-AUDIT-2026-M3-002 自报独立观察,本审计未独立再核(store.rs 文件长,需独立子代理审)| 待 SEC-AUDIT-2026-M3-002 自证 |
| `cargo test --workspace` 146 passed | 本审计跑相同命令确认 | ✅ 一致 |
| `cargo clippy --workspace --all-targets -- -D warnings` EXIT 0 | 本审计:`cargo clippy -p partisync-hub --all-targets -- -D warnings` FAIL(1 error)| ⚠️ **与 SEC-AUDIT-2026-M3-002 不一致**——SEC-AUDIT-2026-M3-002 跑的是 workspace 全集(可能未受 hub 工作树事故影响),本审计只跑 hub,**仅 hub 受新代码影响**。两者**不矛盾**,但 SEC-AUDIT-2026-M3-002 报告应注明"未涵盖 hub crate 工作树新代码" |
| D1 可实质清偿 | 本审计:在关闭 §四 工作树事故 + §六 deny 失败 + §三 P7 交接点论证后,D1 闭环可宣告 | 维持(但有前提) |

---

## 九、综合签核建议

### 9.1 现状建议

**不建议在当前状态下对 WP01-T03 验收或重启 T03+ 任务**。理由:
1. 工作树事故(P0):hub 现有未提交实现不通过 tests + clippy;
2. deny 门禁不绿(P1):与 ADR-0011 签核前提矛盾;
3. P7 不变量交接完整证明缺失(P1):WP03 规格未起草。

### 9.2 重启建议(按序)

1. **本周必做**(P0+P1 关闭):
   - 提交当前 hub 工作树为 `M3-WP01-T03`(如实现尚未达验收,降级为 `M3-WP01-T03-wip`,并修复 tests + clippy);
   - 单独任务 `M3-WP01-T02a`:解决 `cargo deny check licenses/bans` 失败(在 deny.toml 中补 CDLA-Permissive-2.0 + 修复 wildcard);
   - 单独任务 `M3-WP03-T00`(起草 WP03 规格):在 G0 批准前明确 §三 三要素。
2. **下周建议**(P2 关闭):
   - ADR-0011 修订 1:补 sled 1.0.0-alpha.124 事实注脚;
   - M3-WP01 §4 补"256 行上限依据"段;
   - SEC-AUDIT-2026-M3-002 自证 `pairing_session.shared_secret` 落盘路径(store.rs:1834-1852 段独立再审)。
3. **持续登记**(P3):
   - cargo audit/deny 数据基数差异(1251 advisories vs 406 deps)需厘清;
   - 跨会话审计协同:建议下次 M3 关键节点(每 WP G0 批准)均触发独立 AI 审计,与治理面/密码学面双轨。

### 9.3 D1 清偿最终结论

承接 SEC-AUDIT-2026-M3-002 §一"Pass with Caveats":

- **D1 可实质清偿**(前提条件:本审计 §九.2 第 1 项三项关闭);
- **不延期**:M3 早期不必再委托第三方外部密码学公司,前提是 M3 接入 D2(空间供给流程 kdf_salt 接线)时由 SEC-AUDIT-2026-M3-002 审计员(或同等级独立 AI)做**局部密码学复核**(单 PR ≤400 行,审计范围:新接线面)。

---

## 十、本次审计未覆盖(显式声明)

- 未独立再核 `pairing_session.shared_secret` 落盘路径(SEC-AUDIT-2026-M3-002 自报);
- 未审 WP02–WP06 规格(尚未起草);
- 未跑 `cargo xtask trace M3-WP01-*` 追溯;
- 未实测 proptest 均匀性(归 T03–T06 验收);
- 未比对 M2-WP09 混沌床与 M3-WP01 §5 失效注入是否兼容;
- 未独立再核 crates.io 上 sled 1.0.0-alpha.124 的发布日期(只确认版本号与维护活跃度);
- 未审 `crates/partisync-hub/tests/wp01.rs` 内容(7 处宏错误由编译器直接诊断,未读代码逻辑)。

---

## 审计员签核

- 审计员:独立 AI 审计员(无项目先验上下文,从零读起,接续 SEC-AUDIT-2026-M3-001/002 链式审计)
- 审计日期: 2026-09-20
- 输入:`docs/specs/M3-WP00.md`、`docs/specs/M3-WP01.md`、`docs/specs/M2-WP03.md`、`docs/adr/0011-fjall-metadata-lsm.md`、`docs/reviews/SEC-AUDIT-2026-M3-001/002.md`、`docs/reports/M2-report.md`、`AGENTS.md`、`crates/partisync-hub/src/{lib,encode,entry_plane,shard}.rs`、`crates/partisync-hub/Cargo.toml`、`Cargo.toml` workspace 段、`cargo check / clippy / test / audit / deny / info sled` 输出
- 输出:本报告 + (SEC-AUDIT-2026-M3-002.md 由专项子代理写盘)
- 范围限制:仅 M3 早期治理/密码学/工作树状态三层;WP02+ 规格不在范围

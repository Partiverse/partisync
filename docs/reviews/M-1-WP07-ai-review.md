# AI 对抗审查报告：M-1-WP07（core::ulid）

- 审查者：zcode/GLM-5.3-Flash（对抗角色，SOP 执行方案 §3.1-S6 第一通道）
- 审查对象：commit `1c3a70f`（测试先行）→ `361a1c8`（实现）的累积 diff，
  即 `crates/partisync-core/src/ulid.rs`、`tests/ulid.rs`、`Cargo.toml`
- 审查基线：SPEC docs/specs/M-1-WP07.md、ADR-0001、properties.md L1–L3、AGENTS.md 红线
- 方法：逐行对照规格 → 边界枚举（溢出/掩码/长度/字符集/排序）→ 测试有效性检查
  （每条测试问「若实现相反，它会红吗」）→ API 查证核对（getrandom 0.2.17 源码）

## 发现清单

| # | 严重度 | 位置 | 描述 | 处置 |
|---|---|---|---|---|
| F1 | **中** | `FromStr::from_str` | 溢出判断误写为 `value == 0 && d > 7`，会把**非首字符**的合法大译值误判为 Overflow（如 `"008…"` 合法 = 8·2^118）。既有属性测试全走 `from_parts→encode→parse` 路径，编码输出首字符必 ≤7，**测不出此缺陷**——规格符合性缺口 | **S5 期间修复**：改为 `i == 0` 判定；补回归测试 `overflow_applies_to_first_char_only`（正例 "008…"/"088…" + 反例 "8ZZZ…"） |
| F2 | 低 | `Ulid::now` | 系统时钟早于 Unix 纪元时静默饱和为 0，未文档化；审计者无法从代码判断这是有意行为还是疏漏 | **已修复**：doc comment 显式文档化（与 SPEC「熵失败 panic」并列为时间源语义） |
| F3 | 提示 | `tests/ulid.rs::is_crockford` | `matches!(b, I/L/O/U)` 条件冗余（字母表本就不含），保留作为文档性断言 | 接受（注释价值 > 噪声） |
| F4 | 提示 | `Ulid` | 未实现 serde/UUID 互转；内部主键当前无序列化需求 | 接受（SPEC 非目标；出现需求时另行 ADR） |
| F5 | 提示 | 测试笔误 ×2 | 红阶段 `length_must_be_exactly_26` 的 27 字符构造错误（28 字符）；绿阶段 `中` 后误写 26 个零（27 字符，长度检查先于字符检查触发，掩盖了目标断言）——**均被测试自身红/失败暴露并修正**，属 S3/S5 循环正常损耗 | 已修复 |

## 测试有效性抽查（防「假测试」，执行方案 §4.6 变异思维的静态代偿）

- `zero_vector`/`max_vector`：若掩码方向写反或进位错，必红 ✓
- `overflow_vector`：仅覆盖首字符溢出；F1 修复前「非首字符误判」无测试覆盖——已由 F1 回归测试补齐 ✓
- `prop_roundtrip`：随机段取任意 u128（含超宽），能抓掩码缺失 ✓；**但抓不到 F1**（见上），说明「属性测试 ≠ 规格完备」，
  变异测试基建（M-1-WP04，债务登记）落地前需靠对抗审查人工枚举边角
- `prop_ordering_by_timestamp`：diff ≥1 随机段任意，能抓「按 random 优先比较」类实现错误 ✓

## API 查证核对

- `getrandom::getrandom(dest: &mut [u8]) -> Result<(), Error>`：经 `~/.cargo/registry/src/.../getrandom-0.2.17/src/lib.rs:370` 源码确认（AGENTS「查证再写」规则）✓
- `u128::to_be_bytes` 用于 `const fn`：编译器验证 ✓
- workspace `unsafe_code = "forbid"`：diff 无 unsafe ✓

## 结论

**有条件通过**：F1 为真缺陷且已在 S5 闭环（修复+回归测试），F2 已文档化。
SPEC 验收标准逐条核对全部满足（14 测试含 3 属性测试全绿、零运行时依赖除 getrandom、
fmt/clippy -D warnings 通过）。建议人工终审（G2 第二通道）复核 F1 的修复面。

签字：AI-Review: zcode/GLM-5.3-Flash · 2026-09-18

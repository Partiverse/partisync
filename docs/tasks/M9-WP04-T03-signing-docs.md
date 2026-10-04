# Task: M9-WP04-T03 签名流程文档 + 公钥文档增补 + SPEC/WP00 状态回填（WP04 关账）

> **范围外**：示例扩展生产钥签名 = 密钥持有人动作（本 PR 仅登记人工项，
> 沿 ADR-0027 待批准项②「密钥生成仪式留实跑期」判例）；不操控 GUI（本
> WP 无桌面 UI 面）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP04-T03 |
| **类型** | 文档收尾（R0 纯文档 PR；WP04 关账盘点） |
| **优先级** | P0（SPEC §3 任务表 T03；M9-WP00 §2 拍板项③ 勾销落点） |
| **范围** | `docs/release/EXT-SIGNING.md`（分发者签名命令链）+ `RELEASE-PUB-KEY.md` 扩展签名节增补 + SPEC M9-WP04 §4 验收勾选（关账对照）+ M9-WP00 §1-WP04/§2 ③/§4 G6 状态回填 + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | docs/specs/M9-WP04.md §2.3/§3/§4 + docs/adr/0030-extension-signing-toolchain.md + docs/release/key-custody.md「CI 签名口径」 |

## 契约落点

1. **EXT-SIGNING.md（SPEC §2.3）**：minisign CLI 签 `.wasm` 的可复制
   命令链（sign → 装载通过；tamper → `BadSignature` 拒绝的端到端自验）、
   trusted comment 约定（CI 子钥 / 主钥应急双口径）、分发物三文件
   约定、轮换/撤销引用 key-custody.md。
2. **RELEASE-PUB-KEY.md 增补「扩展签名」节**：复用同双钥声明（主钥
   `EBC32789A716D70A` + CI 子钥 `E056CBB62BF3EF34`）+ ext-host 装载期
   强制验签语义（任一钥通过即有效 / `allow_legacy=false` / 无豁免）。
3. **SPEC §4 勾选（关账对照）**：T02 五路探针 / 存量零回归 / deny 实证
   / 依赖面最小 + T03 文档端到端 / 状态回填 + 无桌面 UI 面 + WP 收尾
   盘点——逐项勾选并挂 PR 证据。
4. **M9-WP00 回填**：§1 表 WP04 行收官状态；§2 拍板项③「扩展签名选型」
   ⏸ → ✅（ADR-0030 随 SPEC #142 批准生效）；§4 债表 G6「扩展供应链
   签名/registry」→ ✅ 已清偿（装载期强制验签 P21 转正；registry/更新
   通道维持「不自建」议题登记）。

## 边界与人工项

- **人工项（持有人动作，不阻塞本 PR 合入）**：生产路径示例扩展
  （`demo_tool.wasm` 的分发形态）需 CI 子钥/主钥签名后方可对外分发——
  持有人签名前，示例扩展在强制验签下不可装载（SPEC §6-R5 预期行为，
  fail-closed 正确姿势）。沿 ADR-0027 待批准项②判例挂实跑期。
- 测试面 fixtures 用 test-only 钥（ext-host tests/fixtures/README.md），
  与生产钥材料零接触——key-custody 纪律不受影响。

## 验收

- [ ] EXT-SIGNING.md 含可复制命令链（SPEC §4「文档端到端」）；
- [ ] RELEASE-PUB-KEY.md 扩展签名节落盘；
- [ ] SPEC M9-WP04 §4 全勾 + M9-WP00 三处状态回填；
- [ ] 本卡落盘（先卡后工）；
- [ ] fmt/clippy 全绿（文档 PR 沿门禁全量跑）。

# Task: M9-WP04-T02 minisign-verify 入根 + 装载期强制验签（P21）+ 五路探针 + fixtures

> **范围外**：签名流程文档与示例扩展生产钥签名（持有人动作）、WP00 状态
> 回填 = T03；不操控 GUI（本 WP 无桌面 UI 面，SPEC §4 明示）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP04-T02 |
| **类型** | 实现（R1：新顶层依赖入根（ADR-0030 承载）+ 公共错误面扩展 + 装载序插桩；PR 沿 M9-WP03-T02 判例同 ID 堆叠拆分 ≤400 行） |
| **优先级** | P0（M9-WP00 §4 债表 G6 清偿主体） |
| **范围** | SPEC M9-WP04 §2.1/§2.2/§4 实装：minisign-verify 入根 + `ExtTool::load` 强制验签接线 + `LoadError::{Unsigned,BadSignature}` + scan 孤儿 `.minisig` 拒 + P21 五路探针（转正）+ fixtures（test 钥 + 预签名 `.minisig`） |
| **创建日期** | 2026-10-04 |
| **来源** | docs/specs/M9-WP04.md（#142 已批准）+ docs/adr/0030-extension-signing-toolchain.md + docs/tests/properties.md P21 行 |

## 契约落点

1. **依赖入根（ADR-0030 兑现）**：根 `Cargo.toml`
   `[workspace.dependencies]` 新增 `minisign-verify = "0.3"`，ext-host
   `[dependencies]` 引 workspace pin——`git diff Cargo.toml` 仅此一项
   （SPEC §4「依赖面最小」）；deny 实证随本地门禁，输出登记 PR 正文。
2. **signature 模块（新增 `src/signature.rs`）**：
   - `ANCHOR_PUBKEYS`：发布双钥 base64 内嵌常量（RELEASE-PUB-KEY.md
     主钥 `EBC32789A716D70A` + CI 子钥 `E056CBB62BF3EF34`，任一通过即
     有效）；
   - `verify(content, sig, anchors)`：逐钥 `pk.verify(content, sig,
     false)`（`allow_legacy=false`），任一通过即放行；
   - `SignatureError::{MissingSig, Io, BadSignature}` 内部面，registry
     侧映射 `LoadError::{Unsigned, BadSignature}`（Display 携带扩展名
     定位）。
3. **装载序插桩（`ExtTool::load`，registry.rs:148）**：manifest 三阶段
   校验 → **验签（新插点）** → preflight → 编译 → 实例化——缺签/坏签在
   component 字节进入 wasmtime 编译器之前拒绝（[P14] fail-closed 同构，
   [P21]）；`ExtRegistry::scan` 经 `register` 同一 load 路径，语义自动
   一致；scan 孤儿 `.minisig`（有签名无 wasm）显式拒（沿孤儿 `.wasm`
   P2-1 判例，归 `LoadError::Scan`）。wasm 字节读取失败保持 `Component`
   归因（编译前置 IO，非签名面）。
4. **P21 五路探针（`tests/wp04_signature.rs`，转正）**：①合法签名装载
   通过；②篡改 `.wasm` 单字节必拒（`BadSignature`）；③缺 `.minisig`
   必拒（`Unsigned`）；④未知钥签名必拒（生产锚 + 测试钥签名 fixture
   = 天然未知钥）；⑤拒绝先于编译（变体断言验签向非 `Component` 向 +
   「签名合法但字节非法 → `Component`」对照，反证放行后确实进编译器）。
5. **fixtures（测试面自足，生产钥材料零接触）**：test-only keypair 入
   仓（pub 注释显式标注非生产钥、无口令、不入任何锚集）+ 预签名
   `demo_tool.minisig` / `probe_deny.minisig` / `spin_loop.minisig` /
   `garbage.wasm+minisig`（对既有 fixture 字节一次性 minisign CLI 签）。

## 设计决策留痕（SPEC 未字面规定，PR 正文报备）

- **`ExtTool::load_with_anchors`（pub，锚集参数化装载入口）**：SPEC §2.1
  锚 = 生产双钥（私钥不可得，key-custody 纪律零接触），而 §2.3/§4 要求
  测试面「test-only keypair 预签名 fixture」+「经测试钥签名路径」零回归
  ——两者合取的唯一闭环 = 验签核心参数化钥集：`ExtTool::load` 恒用内嵌
  `ANCHOR_PUBKEYS`（产品路径无任何开关/env/豁免，P21(c) 字面），探针①
  经 `load_with_anchors` 注入测试钥走全装载序。API 新增面：一处 pub 方法，
  文档注释标注语义；非豁免通道（验签仍强制，仅锚集来源可换——与 ADR-0030
  修订触发「专钥分域」方向兼容）。如需收敛（cfg(test)/隐藏）挂人工拍板。
- **`.minisig` 路径推导**：`wasm_path.with_extension("minisig")`——对
  `.wasm` 与 `.wat`（wasmtime 文本编译面，wp06 既有用法）同名规则一致；
  签名对象 = 传入编译器的同一文件字节。
- **探针④的实现口径**：fixtures 签名均出自 test-only 钥 → 生产锚下即
  「未知钥签名」，`ExtTool::load`（产品锚）直测即闭环，无第二把废弃钥。

## 边界与既有决定

- minisign-verify API 以本地 vendored 源码复核为准（0.3.x：
  `PublicKey::from_base64` / `Signature::from_file` / `PublicKey::verify
  (&self, &[u8], &Signature, bool)`；MIT、零依赖——SPEC §6-R1 双向执行）。
- 既有测试适配面（已逐文件核对）：smoke / integration_p13_p14 /
  probes_p13_p14 / probe_index / manifest_p14 走 `load_component` /
  `load_with_manifest` / 直连 Component——不在插桩面，零改动；
  registry_t04b scan 两处拷贝 `.wasm` 同步拷 `.minisig`（撞名测试验签
  通过后到 `Duplicate`，语义保持）；wp06 直接用 fixture 路径（同名
  `.minisig` 入 fixtures 目录即零改动）。无测试删除/断言放宽。
- 签名产出 = minisign CLI（本机 0.12 一次性生成，不进依赖图，ADR-0027
  决策 1 口径）。

## 验收

- [ ] minisign-verify 入根，`git diff Cargo.toml` 依赖面仅此一项；
- [ ] `ExtTool::load` 装载序 manifest → 验签 → preflight → 编译（拒绝
      先于编译）；scan 孤儿 `.minisig` 显式拒；
- [ ] P21 五路探针全绿（tests/wp04_signature.rs）+ properties.md P21
      转正行；
- [ ] 存量测试零回归（ext-host 全绿；无删除/放宽）；
- [ ] `cargo deny check` 全绿（输出登记 PR 正文）；
- [ ] fmt / clippy -D warnings / test（ext-host crate）本地全绿；
- [ ] 本卡落盘（先卡后工）。

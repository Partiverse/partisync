# 扩展签名与分发流程（M9-WP04 / ADR-0030）

> 状态: 生效（随 SPEC M9-WP04 批准，PR #142；实装 PR #144/#145）。
> 关联: [ADR-0030](../adr/0030-extension-signing-toolchain.md)（选型落
> 锤）· [ADR-0027](../adr/0027-release-signing-toolchain.md)（发布签名
> minisign 线位）· [RELEASE-PUB-KEY.md](RELEASE-PUB-KEY.md)（锚定公钥）
> · [key-custody.md](key-custody.md)（密钥双人保管/轮换/撤销）。

## 信任语义（装载期强制验签）

ext-host 对每个扩展执行**装载期强制验签**（不变量 [P21]）：

- 分发物**三文件**：`<name>.wasm`（component）+ `<name>.json`
  （manifest）+ `<name>.minisig`（对 `.wasm` 字节的 minisign 签名）；
- 装载序：manifest 校验 → **验签** → preflight → component 编译——缺签
  /坏签在 wasmtime 编译器接触字节**之前**拒绝（`LoadError::Unsigned` /
  `BadSignature`），无 unsigned 豁免开关（env/config 均不设）；
- 锚定公钥 = 发布双钥（见 RELEASE-PUB-KEY.md），**任一通过即有效**；
  签名算法走 minisign 默认（非 legacy 预哈希，`allow_legacy=false`）；
- 孤儿 `.minisig`（有签名无同名 `.wasm`）目录扫描期显式拒。

## 签名命令链（分发者）

前置：`minisign` CLI（`brew install minisign` 或
[jedisct1/minisign releases](https://github.com/jedisct1/minisign/releases)；
CI 侧经 secret 注入私钥，见 key-custody.md「CI 签名口径」）。

```sh
# 1. 对扩展 component 签名（-x 指定输出名：分发面要求 <name>.minisig，
#    与装载期的同名推导规则一致）
minisign -S -s <seckey-file> -m my_tool.wasm -x my_tool.minisig \
  -t "PartiSync extension signature (<name> vX.Y.Z)"

# 2. 分发三文件一起投放扩展目录（本地目录放置 = 本 WP 分发形态；
#    registry/更新通道不自建，ADR-0030 §非目标）
#    <ext-dir>/my_tool.wasm + my_tool.json + my_tool.minisig

# 3. 自验（干净环境；-p 公钥文件 = RELEASE-PUB-KEY.md 内嵌 base64 存为
#    partisync.pub，双钥任一验过即有效）
minisign -Vm my_tool.wasm -p partisync.pub
```

**trusted comment 约定**：CI 自动化产物标
`PartiSync extension signature (CI subkey)`；主钥离线应急签名标
`PartiSync extension signature (master key, offline)`——与发布产物签名
口径一致（key-custody.md「CI 签名口径」）。

## 端到端自验（对应宿主行为）

| 步骤 | 命令/动作 | 预期 |
|---|---|---|
| 合法签名装载 | 三文件齐备放入扩展目录 → 启动（`partisync` MCP `ext_list`） | 工具注册成功 |
| 篡改 component | 改 `.wasm` 任一字节 → 重新启动 | 装载拒绝 `BadSignature`（签名对字节绑定） |
| 缺签 | 删 `.minisig` → 重新启动 | 装载拒绝 `Unsigned` |
| 未知钥签名 | 用非锚定钥签名 → 重新启动 | 装载拒绝 `BadSignature` |

测试面同构探针见 `crates/partisync-ext-host/tests/wp04_signature.rs`
（[P21] 五路 + scan 孤儿签名附加；fixtures 用 test-only 钥，见
`crates/partisync-ext-host/tests/fixtures/README.md`）。

## 轮换与撤销

- 密钥轮换/撤销流程见 key-custody.md（生成仪式、口令分持、CI 子钥
  泄漏处置）；旧公钥撤销与替换在
  [RELEASE-PUB-KEY.md](RELEASE-PUB-KEY.md)「撤销公告」节顶登记；
- 专钥分域（extension-dedicated 钥，与发布钥分离）为 ADR-0030 修订
  触发条件：首个第三方分发者出现时修订。

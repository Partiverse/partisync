# Release 公钥与验签说明（M8-WP02）

> 状态: 骨架（公钥在首个密钥生成仪式后填入，见
> `docs/release/key-custody.md` 生成仪式节；签名步启用以 ADR-0027
> 批准为前置）。

## 验证步骤（发布后生效）

```sh
# 1. 安装 minisign（任一渠道）
brew install minisign        # macOS
# 或 https://github.com/jedisct1/minisign/releases

# 2. 下载本文件保存为 partisync.pub（内容见下）

# 3. 验证产物（以 v0.1.0-alpha 为例）
minisign -Vm partisync-cli-linux-x86_64.tar.gz -p partisync.pub
minisign -Vm SHA256SUMS -p partisync.pub
sha256sum -c SHA256SUMS      # 双保险：清单校验
```

## 公钥（双钥：主钥离线 + CI 子钥自动化）

发布签名采用双钥（key-custody.md「CI 签名口径」）：CI 自动化产物由
**CI 子钥**签署（`.minisig` trusted comment 标 `CI subkey`）；主钥离线
双人保管，用于应急/最终签名。验签时任一公钥通过即有效（两钥同列如下）。

**主钥**（指纹 `EBC32789A716D70A`，生成仪式 2026-10-01）：

```
untrusted comment: minisign public key EBC32789A716D70A
RWQK1xaniSfD6+Wn9Qp+/A+WIUQ4P/tUoIrtuQawXZozUeu/BjIst0IU
```

**CI 子钥**（指纹 `E056CBB62BF3EF34`，2026-10-01；无口令，仅 CI secret）：

```
untrusted comment: PartiSync CI signing subkey
RWQ07/MrtstW4BJcQJxvDyj415FpHOt9qD6/5PDzjkQKQF/HDvxgaX4j
```

## 扩展签名（M9-WP04 / ADR-0030）

扩展 component 的分发签名**复用同双钥**（上列主钥 + CI 子钥，任一通过
即有效）：扩展分发物 = `<name>.wasm` + `<name>.json` + `<name>.minisig`
（对 `.wasm` 字节的 minisign 签名），签名命令链与端到端自验见
[EXT-SIGNING.md](EXT-SIGNING.md)。

ext-host **装载期强制验签**（不变量 P21，SPEC M9-WP04 §2.2）：缺签
（`Unsigned`）/ 坏签（`BadSignature`）在 component 字节进入 wasmtime
编译器之前拒绝；`allow_legacy=false`（minisign 默认算法）；**无 unsigned
豁免通道**——手工验签与宿主装载语义一致：

```sh
minisign -Vm my_tool.wasm -p partisync.pub   # 任一钥通过即有效
```

## 撤销公告

（无——若发生轮换，此处置顶登记旧公钥撤销与替换，见 key-custody.md 轮换节。）

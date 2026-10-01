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

## 公钥（待生成仪式后填入）

```
untrusted comment: PartiSync release public key
（minisign -G 产出公钥一行，生成仪式后由双人核对指纹入此处）
```

## 撤销公告

（无——若发生轮换，此处置顶登记旧公钥撤销与替换，见 key-custody.md 轮换节。）

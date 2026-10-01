# ADR-0027: 发布签名工具链选型——minisign（离线工程签名）

版本: 0.1 · 状态: **接受**（2026-10-01 用户指令「批准pr63」回填，随本
PR 合入生效；批准人 @lead——待批准项①销账；待批准项②首个签名密钥
生成仪式留 T02 实跑期按 key-custody.md 双人流程执行）·
关联: SPEC M8-WP02 §2.2（批准，PR #61）、执行方案 §5.4（G4：minisign/
工程签名密钥，密钥双人保管）、M8-roadmap-proposal §5-WP02
负责人: @lead · 批准人: @lead · 起草日期: 2026-10-01

## 背景

G1 发布空白清偿（M8-WP02）需要签名工具链。候选与约束：

| 候选 | 优点 | 缺点 |
|---|---|---|
| **minisign**（执行方案 §5.4 点名） | 极简（一对密钥一条签名）；格式稳定十年；验证端依赖极小 | 生态小（非 X.509 链） |
| signify | OpenBSD 同构 | 工具链在 Rust/CI 侧更冷门 |
| GPG | 通用 | 密钥管理复杂度不成比例；仓库已有 GPG 个人签经验但工程签用它过重 |
| Sigstore/cosign | 生态新 | 依赖外部 OIDC/Rekor 服务——与本地优先立场冲突 |

约束：①产物面是静态二进制/dmg（非容器/包管理器通道，X.509 链收益
低）；②「本地优先」立场要求验证不依赖在线服务；③单人团队 + 双人
保管流程，密钥面越小越好；④新依赖需 cargo deny 通过（铁律 8）。

## 决策（草稿）

1. **签名：minisign**。生成/签名在 CI 用 `minisign` CLI（runner 安装，
   **不进 workspace 依赖图**——deny 零改动）；私钥 GitHub Actions
   secret 注入，**不出 CI**；
2. **验签文档面**：README + `docs/release/RELEASE-PUB-KEY.md` 收录
   公钥与验签步骤（干净环境 `minisign -Vm` 一条命令）；Rust 侧验证
   （如未来桌面壳内验签）用 `minisign-verify` crate（纯验签、零依赖
   链，届时另走 deny 核对，不在本 ADR 入根）；
3. **密钥双人保管**：私钥生成采用 minisign 加密私钥（口令分持两半，
   两名持有人各执一半，重构需同时在场的流程文档见
   `docs/release/key-custody.md`——流程级文档，AI 不接触密钥材料；
   若人力不足时点的降级口径：单持 + 恢复盒密封，登记为风险接受）；
4. **SHA256SUMS 双保险**：minisign 签名 + SHA256 清单并附，清单本身
   亦被签名。

## 后果

- 正面：验证面最小化（一条命令、一个公钥、零在线依赖）；CI 零新
  crate 依赖；与执行方案 §5.4 口径一致；
- 负面/风险：minisign 非 X.509——未来企业客户若强制要求 Authenticode
  /链式签名需增补 ADR（登记触发条件）；macOS 公证（Apple 账号）是
  独立外部依赖，SPEC 已划非目标；
- 中性：sigstore 若生态成为事实标准，M9+ 可作第二签名附（格式不互斥）。

## 待批准项

- [x] 用户拍板 minisign 选型 + 密钥保管流程（本 ADR 转正，2026-10-01）
- [x] 首个签名密钥生成仪式——第 1/2 步完成（2026-10-01，`~/.minisign/
      minisign.key` + 口令分持）；公钥入仓与 CI secret 注入待持有人导出
      公钥后完成（key-custody.md 生成仪式节登记）

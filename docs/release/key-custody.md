# 工程签名密钥双人保管流程（M8-WP02）

> 状态: 骨架（随 ADR-0027 批准生效）· 本文档为**流程级**，不记录任何
> 密钥材料。签署人: @lead + 第二持有人（首个实体密钥生成仪式时登记）。

## 密钥面（ADR-0027 决策 3）

| 对象 | 位置 | 保管 |
|---|---|---|
| minisign 公钥 | 仓内 `docs/release/RELEASE-PUB-KEY.md` | 公开 |
| minisign 私钥（口令加密） | GitHub Actions secret `RELEASE_SIGNING_KEY` | 仅 CI；不可导出读取（GitHub secret 语义） |
| 私钥口令 | 分持两半（A 持前半 + B 持后半） | 线下，各持有人自管 |
| 恢复路径 | CI secret 丢失 → 双人在场用两半口令重生成密钥对 → 新公钥入仓 + 旧公钥标记撤销 | 见「轮换」节 |

## 生成仪式（T02 实跑期执行，一次性）

1. 双人在场，离线机器执行 `minisign -G`（生成加密私钥 + 公钥）；
2. 口令随机生成后即拆两半：A 持前半、B 持后半，各自离线保存
   （口令明文不落盘、不进聊天记录/邮件/截图）；
3. 公钥双人在场核对指纹后由 A 入仓（PR + 第二人 PR review）；
4. 私钥注入 GitHub secret 由 A 执行，B 在场核对 fingerprint 尾四位；
5. 本文件补登记：仪式日期、参与人、公钥指纹（不含任何口令材料）。

**登记（2026-10-01）**：生成仪式完成——密钥对落地（指纹
`EBC32789A716D70A`）、口令分持按第 2 条执行；公钥已入仓
（`docs/release/RELEASE-PUB-KEY.md`）；CI secrets
`RELEASE_SIGNING_KEY`/`RELEASE_SIGNING_PASSWORD` 已注入
（2026-10-01T10:22Z）。参与人登记由持有人在发布报告复核时补签。

## CI 签名口径（2026-10-01 修订；ADR-0027 后果节「企业链式签名」触发前的过渡口径）

- CI 消费两个 secret：`RELEASE_SIGNING_KEY`（口令加密私钥全文）+
  `RELEASE_SIGNING_PASSWORD`（口令）——GitHub secret 语义下不可导出读取；
  口令两半线下分持**不变**（CI 口令仅供自动签名，泄漏即触发轮换）；
- workflow 签名步为**条件执行**：secret 未配置时跳过签名，Release 草稿
  显式标注 unsigned（不假绿）；
- **修订（2026-10-01，触发落地）**：实测 C 版 minisign 0.12 无密码环境
  变量（`get_password()` 强制 tty，CI 不可交互）→ 切换 **CI 专用无口令
  子钥**（`minisign -G -W`，指纹 `E056CBB62BF3EF34`，secret
  `RELEASE_SIGNING_CI_KEY`）；主钥离线双人保管不变（口令分持、应急签名
  口径不变）；公钥双签入 RELEASE-PUB-KEY.md。子钥泄漏即删 secret +
  双公钥表移除该行 + Release 公告。

## 日常原则

- CI 只消费 secret，人不经手私钥；
- 任何「本地签名」需求（应急发布）→ 双人在场解密私钥至内存、签完
  即毁，事后在本文档登记；
- **禁止**：私钥入聊天/邮件/云盘/截图；口令两半由同一人持有。

## 轮换与撤销

- 触发：私钥疑似泄露 / 持有人变动 / CI secret 异常读取；
- 流程：新仪式生成新密钥对 → 新公钥入仓 → Release 页置顶公告旧公钥
  撤销 → 旧 secret 删除；
- 每里程碑关账报告复核一次保管状态（沿外部审计双义务复核判例）。

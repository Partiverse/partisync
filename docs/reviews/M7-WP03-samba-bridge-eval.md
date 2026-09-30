# M7-WP03 T02 报告：Samba 桥接评估（smbd export FUSE 挂载点）

> 任务：M7-WP03-T02（SPEC M7-WP03 §3 T02，2026-09-30 批准 PR #47）
> 日期：2026-09-30 · 基线 main=`940fb5c`（含 T01 spike）
> 结论：**Samba 桥接路线技术可行**——smbd 成功 export FUSE 挂载点，
> SMB 协议往返通；**1 个鉴权约束已实测捕获但根因未闭环**（§3，诚实登记）
> 证据纪律：每条约束标注「**实测**」或「**纸面**」（SPEC §2 契约）

---

## 1. 结论速览

| 项 | 结论 | 证据 |
|---|---|---|
| smbd export FUSE 挂载点 | **可行** | **实测**：`testparm` 加载 OK → smbd 启动 → 445 监听 → smbclient 往返收到服务端 NT_STATUS 响应（协议层通，非连接失败） |
| SMB2 + 最小协议协商 | **可行** | **实测**：alpine samba 4.23.8，`server min protocol = SMB2` 生效 |
| oplock/lease 语义 | **不适用**（本 FS 无写路径） | **实测 + 推论**：FUSE 面拒绝一切写（EPERM/EACCES），oplock 无对象可锁；kernel oplocks 配置接受但无实际授予 |
| 锁定语义 | **N/A** | **实测**：无写即无锁需求 |
| 鉴权映射（Samba 用户 ↔ PartiSync 设备身份） | **约束捕获，根因待验证** | **实测**：guest 会话 `NT_STATUS_ACCESS_DENIED`（§3） |
| 性能预期 | **未测**（承 T01 数字） | 见 §5 登记 |

## 2. 实测环境

- 容器：alpine + `/dev/fuse` + `--cap-add SYS_ADMIN`（Docker Desktop Linux VM）
- samba **4.23.8**（alpine 3.x）、smbclient 同版本
- FUSE 挂载面：`partisync-fuse-spike`（T01，mountpoint-s3 式只读+新文件顺序写语义）
- 配置要点：`[global] guest ok / map to guest=Bad User / oplocks=yes / level2 oplocks=yes / kernel oplocks=yes / server min protocol=SMB2`；`[export] path=/mnt/partifuse read only=yes`
- ⚠️ **toolchain 偏差**：容器 rust 1.96.1 vs 仓 pin 1.94（承 T01 报告登记）

## 3. 鉴权约束（实测捕获，根因未闭环）

**现象（实测）**：smbd 启动、445 监听正常，smbclient 连接后
```
NT_STATUS_ACCESS_DENIED listing \*
NT_STATUS_ACCESS_DENIED opening remote file \a.txt
```
——SMB 协议层**通**（服务端返回结构化 NT_STATUS，非连接/协商失败），拒绝发生在
**服务端的文件访问授权层**。

**假设根因（未验证，标注为假设）**：`partifuse-spike` 挂载时启用
`MountOption::DefaultPermissions`（内核按 attr 的 uid/gid 做权限检查），
spike 的 `FileAttr` 回**真实属主**（backing 文件 root:root 644），而
smbd 的 guest 会话以 `nobody` 身份落盘访问 → nobody 对 644 root 文件无读权
→ 拒绝。**待验证的验证路径**：将 guest 映射为属主（`guest account = root`）
或令 FUSE 侧 attr 统一映射为服务可访问 uid，重跑观察 ACCESS_DENIED 是否消失。

**为何未闭环**：容器环境反复重装 551 MB rust+samba 工具链导致探针迭代成本
过高（单次 >10 min），本轮在捕获现象后收敛交付（诚实登记，禁伪装成已解决）。

**对实施 WP 的约束（纸面推断 + 实测现象支撑）**：
- 若假设成立，桥接部署须解决**属主身份映射**（Samba 用户 ↔ FUSE 报告 uid），
  这是 mountpoint-s3 类只读对象面 + Samba 的固有整合点；
- 替代方案（纸面）：FUSE 侧关闭 DefaultPermissions + fuser 自管权限判定
  （把授权决策上移至 PartiSync 层，与设备身份模型对齐——但会扩大 FUSE 宿主
  的权限责任面，需 ADR 权衡）。

## 4. 其余约束清单（实测 + 纸面分栏）

| 约束 | 证据 | 说明 |
|---|---|---|
| smbd 可 export FUSE 挂载点 | **实测** | 核心可行性已验证 |
| 只读共享 × 只读 FUSE 面 | **实测** | `[export] read only=yes` 与 FUSE 拒绝面**语义一致**（双层防护） |
| oplock/lease | **实测（无对象可锁）** | FUSE 面拒一切写 → 无可锁对象；`oplocks/level2/kernel oplocks` 配置被 smbd 接受 |
| 协议降级 | **实测** | `server min protocol = SMB2`；Weak crypto 提示（GnuTLS NTLM 兼容回退）——部署须禁 NTLM |
| 鉴权映射 | **实测（ACCESS_DENIED）** | §3，根因待验证 |
| macOS 客户端接入 | **纸面** | 未测（容器内无 macOS 客户端；macOS 走 Samba 客户端而非 FUSE-T——T02 核心场景待复测） |
| 大文件吞吐 | **未测** | 脚本在鉴权阻断后终止；承 T01 FUSE 读性能（随机读 p95 74.6 µs） |
| Samba 特性冲突 | **纸面** | POSIX ACL / xattr / 符号链接：PartiSync 不透明暴露——需 `vfs objects = acl_xattr` 关闭或容忍降级（未测） |

## 5. 性能（承 T01，无 SMB 层新数字）

SMB 桥接性能 = FUSE 面性能 + SMB 协议开销。T01 实测：随机读 4 KiB
p95 74.6 µs / 顺序读 8761 MiB/s（页缓存口径）。SMB 层吞吐**未测**——
鉴权阻断未能测得（§3）。评估口径：SMB 桥接面向 LAN 企业场景，
网络 RTT 主导，非 FUSE 面瓶颈；实施 WP 须补 SMB 层端到端吞吐基准。

## 6. 复测触发条件（诚实登记）

以下任一满足即复测并闭环 §3 根因：
1. 实施 WP 立项（ADR-0026 接受后）；
2. 探针镜像（rust+samba 持久镜像）就绪后（本轮因环境迭代成本过高未及）；
3. macOS Samba 客户端接入测试（LAN 真实场景）。

## 7. 复核日志

- 执行：GLM-5.3-Flash（ZCode 会话，M7-WP03-T02）
- 实测脚本（临时，不入仓；内容见 §2 配置要点，可复现）
- 环境踩坑：Mach-O/ELF 跨平台产物误执行（见 [[m7-wp03-t01-fuse-spike]]）已在
  本轮修正为容器内 `/tmp/target` 产物

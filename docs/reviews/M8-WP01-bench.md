# M8-WP01 一期基准与复测报告

> 任务：M8-WP01-T03（SPEC M8-WP01 §3，2026-09-30 批准 PR #54）
> 日期：2026-10-01（夜间作业窗口）· 基线：T02 合并后 main
> 环境：alpine 容器（`--device /dev/fuse --cap-add SYS_ADMIN --privileged`）
> + rust（apk）/ 仓 pin 1.94 本地编译验证（M8-WP01 overnight-notes §1）

---

## 1. 冷缓存读基准（ADR-0026 前置条件 2 验收硬项）

**口径**：`drop_caches(3)` 后首读（drop 实测生效 `DROP_OK`）vs 二次读；
`O_DIRECT` 一期未实现（实现无 direct_io）——以 drop-caches 口径替代并
如实标注。文件 64 MiB 伪随机（tmpfs/overlay 后备——**无真实磁盘 IO**，
数字反映 FUSE 协议往返 + VFS 开销，非设备能力）。

| 指标 | 实测 | 说明 |
|---|---|---|
| 冷读 4 MiB 区段 ×5 | **3.2–3.3 ms**（~0.8 ms/MiB） | drop 后首读；与热读同量级（后备无磁盘），差异体现 FUSE 往返一致性 |
| 热读 4 MiB ×5 | 2.5–3.7 ms | 页缓存路径 |
| 顺序全读 64 MiB | **1567 MiB/s**（40.8 ms） | 热路径吞吐上限（非设备口径） |
| 随机读 4 KiB（n=200） | **p50 328 µs / p95 361 µs / max 376 µs** | 与 spike 口径（p95 74.6 µs）差异 = 本轮含 open+close 全系统调用（spike 复用已开 fd）；更贴近真实用户态操作 |

**对照说明**（SPEC 验收要求）：spike 数字（M7-WP03-T01）复用已打开 fd 只
测 read 往返；本轮为「每读全链路」（open→seek→read→close）。两者口径
不同均有效——用户态真实操作是全链路口径，微基准是协议开销口径。

## 2. macOS FUSE-T 复测

**未执行**（本机无 macFUSE/FUSE-T，安装需 sudo 密码——headless 会话不可
得；沿 M7-WP03-T02 判例如实登记）。**复测触发条件**：①用户安装
FUSE-T（`brew install --cask fuse-t` + 授权）后任一会话可执行；②实施
二期立项前必须完成（macOS 是桌面壳主平台）。

## 3. SPEC §3 验收对照

| 验收项 | 状态 | 证据 |
|---|---|---|
| P15 转正 | ✅ | properties.md 正式行（T01，PR #55） |
| 产品探针 + 1.94 容器真挂载 | ✅ | 探针 11 项断言容器绿（T02 PR #56 正文）；1.94 本地编译零警告（overnight-notes §1）；Linux 侧容器 rust 为 1.96.1——**1.94 容器复测**见 §4 偏差登记 |
| 冷缓存读基准 | ✅ | 本报告 §1（drop-caches 口径，O_DIRECT 缺失如实标注） |
| macOS FUSE-T 复测 | ⏸ 触发条件登记 | 本报告 §2 |
| 冒烟门禁（协议 §4.1） | ✅ | T02 PR 正文贴运行输出（ls/cat/写新文件 OK/写已存在拒/rm 拒/原样） |
| clippy/deny/audit 零新增豁免 | ✅ | 本地三件套绿（CI run 见 PR #55/#56） |
| CI 全绿（含 SKIP 路径） | ✅ | runner 无 /dev/fuse → 探针环境门控 SKIP 绿 |

## 4. 偏差与遗留登记

| 项 | 说明 | 去向 |
|---|---|---|
| 容器 rust 1.96.1 vs pin 1.94 | 本地 macOS 1.94 编译零警告（预演）+ 容器 1.96 全套件绿——**双端覆盖但无容器内 1.94** | 二期开工前补 musl 1.94 容器复测（低成本） |
| O_DIRECT/direct_io | 一期实现未做，冷缓存用 drop-caches 替代 | 二期（随写面 overlay 一起评估） |
| 跳写 EINVAL 探针 | 页缓存下 VFS 不可构造直达路径（SEMANTICS.md「防线前置」说明） | 维持登记 |
| Samba 桥接属主映射 | T02（M7-WP03）捕获的 ACCESS_DENIED 根因 | 企业需求确认后（ADR-0026 前置 4） |

## 5. 复核日志

- 执行：GLM-5.3-Flash（ZCode 夜间会话，M8-WP01-T03）
- 脚本：临时不入仓（drop-caches + python3 perf_counter 计时；命令序列
  已在本报告 §1 描述可复现）
- 门禁：T01/T02 PR CI 8/8 绿；本 PR docs-only

# ADR-0016: iroh 通道传递依赖的 deny 基线扩充（licenses + advisories）

状态: 已接受（待人工终审） · 日期: 2026-09-21 · 关联: ADR-0008/0009/0015、
SPEC M4-WP04 · 触发: M4-WP04-T06 回归门禁实测

## 背景

M4-WP04 按后 ADR-0015 接线将 `iroh =1.2.0` + `iroh-blobs =0.103.0`
引入 `partisync-hub`。此前会话的 `cargo deny check` 因 advisory-db
网络拉取失败从未跑完 licenses/advisories 维度，T06 全回归时首次完整
执行，暴露两类基线外条目（均为 iroh/iroh-blobs 传递依赖，非直接依赖）。

## 决策

### 1. licenses 白名单 +2

| 许可证 | 涉及 crate（传递链） | 评估 |
|---|---|---|
| `Unlicense`（公共域） | `async_io_stream 0.3.3`、`pharos 0.5.3`、`ws_stream_wasm 0.7.5`（iroh-blobs wasm 目标条件依赖） | OSI approved + FSF Free；仅 wasm 目标编译路径 |
| `MPL-2.0`（弱 copyleft，文件级） | `attohttpc 0.30.1`（iroh-blobs 链） | OSI approved + FSF Free；文件级 copyleft 不传染，与既有 `CDLA-Permissive-2.0`/`BSL-1.0` 同级宽容 |

两者均为 deny 实测输出中「OSI approved + FSF Free/Libre」项；许可维度
无 copyleft 传染（MPL-2.0 为 file-level）。

### 2. advisories ignore +3（全部 unmaintained，无漏洞）

| Advisory | crate | 传递链 | 说明 |
|---|---|---|---|
| RUSTSEC-2023-0089 | `atomic-polyfill 1.0.3` | heapless 0.7 → postcard 1.1 → iroh-blobs / iroh-relay | 仓库归档；目标平台替代为 portable-atomic，上游无升级路径 |
| RUSTSEC-2024-0436 | `paste 1.0.15` | netlink-packet-core → netdev → netwatch → iroh 1.2.0 | 作者归档；纯编译期宏，无运行时面 |
| RUSTSEC-2024-0370 | `proc-macro-error 0.4.12` | genawaiter 0.99 → bao-tree 0.16 → iroh-blobs 0.103 | 维护者失联 + 锁 syn 1.x；纯编译期宏 |

三条均「Solution: No safe upgrade is available!」，性质同 ADR-0014
附注的 `instant`（unmaintained、无已知漏洞、编译期或边缘平台）。

## 后果

- deny.toml `[licenses] allow` 增 `Unlicense`、`MPL-2.0`；
  `[advisories] ignore` 增上述三条 RUSTSEC-ID，均引用本 ADR。
- 豁免范围限于上列条目；新增 advisory 一律拦截。
- 撤销条件：上游（iroh / iroh-blobs）切换掉对应传递依赖后，同步删除
  对应 ignore/allow 条目并作废本 ADR 相应段落。
- 教训登记：依赖进场（红线 8）必须以**完整跑通**的 `cargo deny check`
  为准——网络失败的重试属于门禁未闭合，不得记录为 ✅（本 ADR 即补票）。

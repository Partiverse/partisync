# WP06 内部渗透测试报告（M4-WP06-T05）

> 报告编号: PEN-M4-WP06-001
> 日期: 2026-09-23
> 关联: `docs/specs/M4-WP06.md` §裁定 3 + `docs/security/threat-model-mcp-gateway.md` + `docs/security/pentest-rfc.md`
> 测试范围: MCP 5 工具 + CLI 入口（partisync-gateway stdio JSON-RPC + sqlx 0.9）
> 测试方法: 18 个内部探针（`crates/partisync-gateway/tests/pen_test.rs`），全部用真实
>   `partisync-mcp` 子进程（与 `mcp_e2e.rs` 同模式）

## 1. 执行摘要

| 维度 | 数值 |
|---|---|
| 探针总数 | 18 |
| 通过（服务端未崩溃） | 18 (100%) |
| 高危发现 | 0 |
| 中危发现 | 0 |
| **P1 发现** | **3**（巨大批次 / 跨库越权 / 缺失 c2pa 字段形态） |
| P2 发现 | 0 |
| 待外部 vendor 复核 | 18 探针全部（独立验证） |

## 2. 探针结果明细

### 2.1 注入探针（8）

| ID | 探针 | 通过 | 发现 |
|---|---|---|---|
| pen_inject_sql_in_query | asset_search query 字段 SQL 注入试探（4 种 payload） | ✅ | — |
| pen_inject_sql_in_content_id | asset_read content_id 字段注入试探（3 种） | ✅ | — |
| pen_inject_huge_string | 1 MiB 单字符串 | ✅ | — |
| pen_inject_unicode_control | Unicode 控制字符 + 双向覆盖 + BOM/零宽（3 种） | ✅ | — |
| pen_inject_empty_string | 空字符串 / 纯空格 / 空白（4 种） | ✅ | — |
| pen_inject_type_mismatch | limit 字段传 string（类型错） | ✅ | — |
| pen_inject_huge_batch | 1000-op asset_organize 批次 | ✅ | **P1**: 不拒收巨大批次（DoS） |
| pen_inject_special_chars_in_tag | tag.value 路径穿越/控制字符/HTML（4 种） | ✅ | — |

### 2.2 鉴权探针（4）

| ID | 探针 | 通过 | 发现 |
|---|---|---|---|
| pen_authz_unknown_content_id | 跨库 content_id → 200 空对象？ | ✅ | — |
| pen_authz_dataset_export_arbitrary | dataset_export 跨库 ID（3 个混合） | ✅ | — |
| pen_authz_organize_unknown_content | asset_organize 对不存在的 content_id | ✅ | **P1**: 不校验 content_id 存在性 |
| pen_authz_output_dir_traversal | output_dir 路径穿越 `/tmp/../../etc/` | ✅ | — |

### 2.3 DoS 探针（3）

| ID | 探针 | 通过 | 发现 |
|---|---|---|---|
| pen_dos_huge_limit | limit=1B 是否被 clamp | ✅ | — |
| pen_dos_shard_size_zero | shard_size=0 + 单记录 | ✅ | — |
| pen_dos_concurrent_requests | 100 次连续调用 5s 超时 | ✅ | — |

### 2.4 C2PA 边界探针（3）

| ID | 探针 | 通过 | 发现 |
|---|---|---|---|
| pen_c2pa_invalid_not_throw | c2pa stage state=Invalid 校验后 asset_read | ✅ | — |
| pen_c2pa_missing_manifest_assets_read | 无 c2pa stage 时 asset_read 返回形态 | ✅ | **P1**: 应为 null 而非字段缺失 |
| pen_c2pa_extreme_detail_size | 1 MiB detail 不爆内存 | ✅ | — |

## 3. P1 发现详解

### Finding #1: asset_organize 不拒收巨大批次（DoS）

**证据**: `pen_inject_huge_batch` —— 1000 个 op 数组被服务端正面接纳。

**影响**: 攻击者可发送 1 万条 op 请求，触发单事务批写阻塞所有后续请求；
1 万 × 100 字符 tag ≈ 1 MB 内存压力 + 长事务 → 其他客户端 hang。

**建议修复**:
- `asset_organize` 入参校验：operations.len() ≤ 100（与 MCP 工具规范上限对齐）
- 过大返回 `ErrorData::invalid_params("operations batch too large")`

**优先级**: P1（DoS 防护）

**责任人**: TBD（本 WP 不修，独立任务）

### Finding #2: asset_organize 不校验 content_id 存在性（AuthZ）

**证据**: `pen_authz_organize_unknown_content` —— 对 `c-does-not-exist` 写 add_tag 返回成功。

**影响**: 攻击者可在不读取 DB 的情况下写入任意 tag 关联（即使是空关联）；间接产生垃圾数据，
可能干扰后续审计 / GC。

**建议修复**:
- 写操作前 `SELECT 1 FROM content WHERE id = ?` 校验
- 不存在返回 `ErrorData::invalid_params("content_id not found")`

**优先级**: P1（数据完整性）

**责任人**: TBD

### Finding #3: absent c2pa stage 字段形态（API 一致性）

**证据**: `pen_c2pa_missing_manifest_assets_read` —— `sidecar_stages.c2pa` 在缺失时
返回**字段缺失**而非 `null`，与其他 stage（如 `ocr`, `embed`）的「字段必有、值可 null」
约定不一致。

**影响**: API 消费者需对 c2pa 字段做 missing-or-null 双重判断；体验差。

**建议修复**:
- mcp.rs asset_read 处补 `c2pa: stages.c2pa.as_ref().map(|s| s.status.clone()).unwrap_or_default()`
  改为 `c2pa: stages.get("c2pa").cloned().unwrap_or(serde_json::Value::Null)`

**优先级**: P1（API 一致性，但低风险）

**责任人**: TBD

## 4. 缓解已生效（无需修复）

| 探针 | 现状 | 备注 |
|---|---|---|
| SQL 注入（query/content_id） | sqlx 0.9 参数化绑定 | WP02 阶段已验证 |
| 1 MiB 超长字符串 | tantivy query 解析无 panic | 内置兜底 |
| Unicode 控制字符 | tantivy BM25 词项分析过滤 | |
| 空查询 / 纯空格 | tantivy 返回空 hits | |
| type-mismatch | serde JSON 反序列化失败 → protocol-level invalid_params | 已生效 |
| dataset_export 跨库 ID | SQL JOIN 自然过滤 | |
| output_dir 路径穿越 | 实际未逃逸（assert） | 但建议加显式 canonicalize |
| 超大 limit | `limit.clamp(1, 100)` | mcp.rs:497 |
| shard_size=0 | 单文件（已实现流写） | WP03 T04 |
| C2PA Invalid / 超长 detail | tantivy / sqlx 不爆内存 | WP05 已验证 |

## 5. 与外部渗透的关系

本报告作为外部 vendor 渗透的**预演**（`pentest-rfc.md` §3.1）：
- 内部已发现 3 项 P1 → 修复后 vendor 测试应跳过
- 其余 15 项 vendor 独立验证（可确认或反驳本报告结论）

外部 vendor 报告独立归档 `docs/reviews/SEC-PENTEST-M4-WP06-{vendor}-{date}.md`。

## 6. 修复时间线建议

| Finding | 建议任务卡 | 优先级 |
|---|---|---|
| #1 巨大批次 | M4-WP06-P1-batch-limit 或合并入 M5 P0 | P1 |
| #2 跨库越权 | 同上（asset_organize 加固任务） | P1 |
| #3 c2pa 字段形态 | 同上（asset_read 字段一致性） | P1 |

不在 WP06 范围（铁律 9「不顺手修」）—— 开独立任务。
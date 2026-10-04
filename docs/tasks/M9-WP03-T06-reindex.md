# Task: M9-WP03-T06 检索索引重建命令（SPEC v0.3 §2.6）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T06 |
| **类型** | 实装（CLI 子命令；R1 全审——产品搜索首个写入方） |
| **优先级** | P0（产品搜索从未工作过的补全件） |
| **范围** | SPEC M9-WP03 v0.3 §2.6 + §3 T06 验收 + §5 cli 三文件 + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | T05 交付期发现（`upsert_content`/`rebuild_index` 零生产调用方）+ 用户批准执行（2026-10-04，索引语义拍板 = 方案 2） |

## 交付物

1. **`partisync reindex`** 子命令（crates/partisync-cli/src/reindex.rs +
   main.rs 接线 + Cargo.toml sqlx workspace dep）：graph entry×content
   全量扫描 → 文本筛选（mime `text/*`/结构化 mime；NULL 时扩展名白名单
   嗅探）→ **CAS 原文读出** → BM25 `ocr_text` 通道 upsert（256 KiB
   char 边界截断；非 UTF-8/非文本跳过计数）→ commit。幂等（upsert 先
   删同 content_id 旧文档）。缺省路径沿 search_cmd 口径（index =
   data_local/.partisync/index——与桌面 T05 对齐目录一致）。
2. 单测：文本索引 + 二进制跳过 + marker 词 CAS 原文命中 + mime/扩展
   嗅探表；OnCommitWithDelay 显式 reload 判例沿 bm25.rs:319。

## 偏差登记

- cli Cargo.toml 增 `sqlx = { workspace = true }`：entry×content 扫描
  SQL 在组合层直写（store 无现成全量方法）；workspace 既有 pin、经
  graph 传递已在用，非新增顶层 crate（铁律 8 免 ADR，任务卡留痕）。
- **数据源两度修正（实施期实证）**：① indexer 对小文件不写 CAS
  （content_id 仅哈希身份，字节在源盘）→ 数据源 = 源文件优先（经
  `--source-root`/jobs 最近根解析 vpath）+ CAS 单块回退；② vpath 前导
  `/` 是虚拟层级非磁盘绝对路径（is_absolute 误判实测踩坑）——恒按根
  相对 join。
- **新债登记：CAS 内容重组缺口**——大文件 chunk hash 列表未持久化
  （chunk_root 仅 Merkle 根，无列表表），content→bytes 不可重组；
  reindex 计 read_errors，修复挂独立任务。

## 真实数据端到端（2026-10-04 实测）

- 播种 wp06_corpus（50 篇 markdown 笔记）→ `partisync reindex`：
  **indexed=50** skip_binary=186 read_errors=2（历史 CAS 缺块，预期）；
- `partisync search "argon2 migration" --mode bm25`：**2 条命中**
  （产品搜索首次真正工作；桌面读取同一 IndexEngine 目录）。

## 验收

- [x] 单测 2 例绿（文本索引/二进制跳过/原文命中 + 判定表）；
- [x] 真实数据集 `partisync reindex`（indexed=50）+ `partisync search "argon2 migration"` 命中 2 条（CLI 端到端）；
- [x] fmt/clippy/test 三件套全绿；零新增顶层 crate；
- [x] 提交挂 Task-ID `M9-WP03-T06`。

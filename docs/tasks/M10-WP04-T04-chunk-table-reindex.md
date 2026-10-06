# Task: M10-WP04-T04 CAS 内容重组清偿（chunk hash 落表 + reindex 重组回退）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP04-T04 |
| **类型** | 实装（NB5 债清偿；R1 全审——schema 迁移 + 索引写入面） |
| **范围** | SPEC M10-WP04 §2.2 + schema.sql v18 + indexer.rs + store.rs 读面 + reindex.rs + properties.md P22 + 本卡 |
| **创建日期** | 2026-10-06 |
| **来源** | M9-WP03-T06 新债登记（indexer.rs:192 弃 hash 列表）+ M9-report §5 债表 ⏳ 行 |

## 交付物

1. schema v18：`content_chunk(content_id, seq, chunk_hash,
   PRIMARY KEY(content_id, seq))` additive 幂等（沿 v16/v17 判例）。
2. indexer.rs:192 站点改取 `put_chunks` hash 列表（cas/store.rs:248
   返回 `(Vec<String>, String)`）落表，与 entry.batch 同事务；写入
   幂等 INSERT OR IGNORE；「内容未变跳过重分块」逻辑不回退。
3. 读出面：`content_chunk_hashes(content_id)`（seq 升序）+ 重组
   helper——CAS 取块 concat + `blake3==content_id` 校验 +
   `chunk_root(list)==entry.chunk_root` 对账；缺块显式 Err。
4. reindex 大文件回退（chunk_root 非空）走重组读出；存量无清单维持
   read_errors（不可回填，诚实登记）；reindex.rs:86 债注释摘除。
   properties.md P22 登记（**登记 PR 先于测试代码**，沿 M9-WP02-T02
   判例）。

## 探针

P22-a 落表对账（list↔chunk_root 相等）；P22-b 重组 roundtrip；
P22-c 人为删块显式 Err；reindex marker 词大文件命中；存量无清单
read_errors 计数不静默。

## 验收

- [x] §3 T04 探针绿（P22-a/b + reindex 重组 marker 命中 / P22-c 删块·删清单行·非法 hash 三注入显式 Err / 存量无清单 read_errors=1 / FK 级联回归探针 t04_chunked_content_cleanup_survives_manifest_fk，crates/partisync-cli/src/reindex.rs tests 4 例）；
- [x] fmt/clippy/test 三件套绿（2026-10-07 本机实跑：`cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` exit 0；cargo deny check 4 项 ok）；零新增顶层依赖；同步面 src 零 diff（仅 sync 测试 FileInsert 构造点机械补 `chunk_hashes: Vec::new()` 一行）；
- [x] 提交挂 Task-ID `M10-WP04-T04`。

## 对抗评审修复登记（2026-10-07）

- **FK 级联（high）**：v18 DDL 改 `REFERENCES content(id) ON DELETE CASCADE`——清单是 content 派生数据，既有清理面（remove_entry 孤儿清理 / add_entry 换内容）删父行时随行级联清；修复前 FK 787 炸删除路径（t04 回归探针先红后绿锁定）。
- **清单行防御（low）**：`reassemble_content` 入口校验 chunk_hash 为 64 位小写 hex（`is_blake3_hex`），非法行显式 Err——防 `<2` 字节切片 panic 与含 `/`/`..` 行的 CAS object_path 路径穿越读。
- **豁免口径（low，铁律 9）**：crates/partisync-sync/tests/{m2_kpi,convergence}.rs、crates/partisync-index/tests/m5_wp05.rs 三处 `FileInsert` 构造点机械补 `chunk_hashes: Vec::new()` 一行，系共享 struct 加字段的编译必然（src 构造点仅 indexer.rs 一处），评审认定非放水。**后续判例**：任务涉及共享 struct 加字段时，任务卡「范围」须预列测试构造点豁免。

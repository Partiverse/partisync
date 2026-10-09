# Task: M11-WP02-T02 partisync-index 只读打开路径 + P24 登记 + 探针

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP02-T02 |
| **类型** | 实施（新公共 API 面，R1——SPEC M11-WP02 §2.5 全审） |
| **来源** | SPEC M11-WP02 §2.1/§3.0-T02/§3.1 + ADR-0032 决策 2 |
| **创建日期** | 2026-10-09 |

## 交付

1. **P24 登记行**（docs/tests/properties.md，独立 commit 先于测试代码——
   cc23fb0，沿 M9-WP02-T02 判例）
2. `Bm25Index::open_read_only` + `is_read_only`（writer 字段 Option 化，
   4 写方法经 `writer_lock()` 守卫——只读实例写方法返回结构化错误）
3. `IndexEngine::open_read_only`（bm25 只读 + vector 同构包装；
   缺目录 → 结构化错误，区别于 open_or_create 建目录语义）
4. `Bm25Index` 导出面补登（lib.rs，SPEC §2.1 契约面）
5. 探针 crates/partisync-index/tests/m11_wp02_readonly.rs ×4
   （P24-a 锁文件不新建 / P24-b 与活跃写者并存 + commit 可见 + 只读
   实例写方法拒 / P24-c 三实例并发 / engine 冒烟 + 缺目录错误）

## 涉及文件清单（Iron Rule 9）

crates/partisync-index/src/search/bm25.rs ·
crates/partisync-index/src/search/engine.rs ·
crates/partisync-index/src/lib.rs ·
crates/partisync-index/tests/m11_wp02_readonly.rs（新）·
docs/tests/properties.md · docs/tasks/M11-WP02-T02-readonly-index.md

## 验收（SPEC §3.1 T02 行）

- [x] 只读打开不创建/获取 `.tantivy-writer.lock`（P24-a 探针）
- [x] 只读实例与活跃 IndexWriter 并存，commit 后 reload 可见（P24-b）
- [x] ≥3 只读实例并发 search 无互斥（P24-c）
- [x] P24 登记行落 docs/tests/properties.md（先于测试代码）
- [x] fmt/clippy/test 三件套绿（clippy -D warnings 零告警；crate 全测
      28+ 用例绿）；改动仅限本卡清单；零新增顶层依赖

# M4-WP01 KPI 底稿（T07 基准与验收报告）

日期: 2026-09-22 · 环境: Apple Silicon (arm64, macOS 25.6.0, 24 GB) ·
release profile（criterion）· 基准: `crates/partisync-ai/benches/wp01_bench.rs`
（`cargo bench -p partisync-ai`）· 去重测量:
`tests/wp01_bench.rs::dedup_savings_unique_counts` · 关联: SPEC M4-WP01
验收标准、ADR-0017（推理与图像栈）

## 1. 口径声明（诚实标注）

- **fake 模型口径**：三推理栈（OCR/转写/嵌入）真模型未冒烟（HF 不可达
  移交登记 §7），本报告所测为**真实代码路径上的非推理部分**——纯 Rust
  stage（缩略图/EXIF）、OCR 后处理（DB 连通域/CTC 解码）、音频重采样、
  fake 嵌入与管线编排（DB 状态机往返）。真实推理吞吐待冒烟后补录；
- criterion 默认样本 100，重负载组 30；时间取中位数（区间见 bench 输出）。

## 2. Per-stage 吞吐（真实代码路径）

| 基准 | 输入 | 中位耗时 | 口径换算 |
|---|---|---|---|
| thumbnail 全链（PNG 解码→512 缩放→JPEG q75 编码） | 1080p PNG | **21.8 ms** | ≈46 张/秒/核 |
| exif 解析（PNG no-exif 早退路径） | 640p PNG | **1.5 µs** | 降级路径开销可忽略 |
| OCR DB find_boxes（连通域，200 文本块） | 960×540 概率图 | **6.0 ms** | ≈46 MP/s 概率图遍历 |
| OCR CTC 解码 | 50 帧 × 6625 类 | **1.4 ms** | det 后识别侧解码开销可控 |
| 转写重采样（线性插值 16 kHz） | 30 s @44.1 kHz | **1.7 ms** | ≈18 000× 实时（转写前处理不构成瓶颈） |
| fake 嵌入（含 ModelManager 校验） | 22 B 文本 | **4.2 µs** | 编排层基准 |

## 3. 管线编排开销（JobSystem v2 状态机）

| 基准 | 中位耗时 | 说明 |
|---|---|---|
| 全终态 rerun（1 content × 5 stage no-op 读路径） | **387 µs** | 重复入队/重跑的地板成本 |
| 全驱动（复位 pending + 5 stage 全驱动，含 spawn_blocking 往返与状态机 UPDATE） | **8.2 ms** | 约 1.6 ms/stage·content，其中推理外开销为主 |

结论：编排开销（SQLite 状态机 + 认领/复位 + blocking 池往返）在毫秒级，
相对真实推理（秒级至分钟级）不构成瓶颈；content 粒度批量驱动
（`run_pending_sidecars`）下摊销可忽略。

## 4. 崩溃恢复压力（SPEC 验收「崩溃恢复」，M4 DoD）

- 形态：`tests/wp01_t06.rs::crash_recovery_stress_random_kill_rounds`——
  **15 轮随机执行点 panic（模拟 kill -9）**，每轮恢复（陈旧作业收口 →
  running 复位 → 续跑）收敛；终态与无故障全量直算**严格等价**
  （done/failed/running 计数全对齐，L5 恢复等价性口径）；
- 单轮恢复在 10 次重试内收敛（实测均 1 次）；无孤儿 running、无重复产物。

## 5. 去重收益（unique 计数）

| 指标 | 值 |
|---|---|
| 合成根（10 组 × 5 份重复 + 70 独立，120 PNG） | entries=**120** |
| 唯一 content（sidecar 入队粒度） | **80** |
| **sidecar 计算节省** | **33.3%**（1 − 80/120） |

与图谱层 unique 口径一致（挂 content 不挂 entry：N entry → 1 content
只算一次 AI）。

## 6. 质量门禁（T07 复核）

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 322 过 0 失败（ai 层 34：wp01 20 + t05 11 + t06 3） |
| `cargo deny check` 四段 | ✅（licenses +NCSA、advisories 网络抖动重试通过） |
| feature 编译矩阵 | ✅ 默认 / ai-ocr / ai-transcribe / ai-embed / **三全开** |

## 7. 真模型冒烟（移交登记，未执行）

| 项 | 状态 |
|---|---|
| 三栈真模型推理实测（ort/fastembed/whisper，含 Metal/CoreML 后端） | ⏸ HF 不可达（curl 000），待网络恢复手动执行 |
| MANIFEST 五条 blake3 填实 | ⏸ 同上（下载后计算回填） |
| OCR 推理口径校正（张量布局/预处理/后处理阈值对真模型输出） | ⏸ 同上；代码按 PaddleOCR 公开口径实现，CI 已钉纯函数行为 |
| 编译链验证 | ✅ whisper.cpp（cmake 4.2 + clang 16）+ ort rc.13 + fastembed 全链 check 过，三 feature 同树共存 |

## 8. SPEC 验收对账（M4-WP01 §验收标准）

| 验收项 | 状态 |
|---|---|
| 崩溃恢复（kill × N 轮 → resume 等价） | ✅ §4（15 轮） |
| content 去重（每阶段恰执行一次） | ✅ wp01::content_dedup + §5 |
| 阶段幂等（产物字节级稳定） | ✅ wp01/t05 幂等重跑测试 |
| 各阶段行为契约（fake 模型 CI 离线） | ✅ 缩略图/EXIF/OCR/转写/嵌入全钉 |
| 模型缺失降级（skipped + 原因可查） | ✅ 单缺/半缺/全缺三口径 |
| 调度接线（scan → 入队 → CLI 可查） | ✅ T06 e2e + sidecar-run/status |
| 回归门禁 / deny / 覆盖率 | ✅ §6 |

## 9. 工件清单

| 文件 | 说明 |
|---|---|
| `crates/partisync-ai/src/sidecar.rs` | 状态机 + SidecarStore + BlobSink |
| `crates/partisync-ai/src/pipeline.rs` | Stage trait + 执行器 |
| `crates/partisync-ai/src/jobs.rs` | v2 编排 + 调度接线 + 本地加载器 |
| `crates/partisync-ai/src/models.rs` | 清单钉版 + ModelManager |
| `crates/partisync-ai/src/stages/` | 五阶段（thumbnail/exif/ocr/transcribe/embed） |
| `crates/partisync-ai/benches/wp01_bench.rs` | criterion 基准（§2/§3） |
| `crates/partisync-ai/tests/wp01*.rs` | 契约/e2e/压力/去重测量 |
| `docs/adr/0017-sidecar-inference-image-stack.md` | 推理与图像栈批次 |

## 10. 开放项

| 优先级 | 项 | 负责 |
|---|---|---|
| P0 | 真模型冒烟三件套（§7：实测 + blake3 回填 + OCR 口径校正） | @lead |
| P1 | OCR 旋转框/多边形 unclip（现轴对齐简化框） | @lead（冒烟后评估） |
| P1 | 音视频容器解封装（现 applicable 仅 Audio） | @lead |
| P2 | sidecar 产物与 WP02 检索的向量读回接线 | WP02 起草对账 |
| P2 | PARTISYNC_MODEL_CACHE_DIR 部署文档化 | M4 收尾 |

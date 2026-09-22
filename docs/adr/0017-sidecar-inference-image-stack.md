# ADR-0017: Sidecar 推理栈与图像栈依赖批次进场

状态: 已接受 · 日期: 2026-09-22 · 关联: SPEC M4-WP01（裁定 3/4/5）、
ADR-0014（依赖批次治理先例）· 替代方案: candle 全家桶、自绑 ONNX
Runtime、云 API 嵌入

## 背景

M4-WP01 Sidecar 管线需要五阶段能力（SPEC M4-WP01 裁定 3）：缩略图/EXIF
（纯 Rust）→ OCR → 音视频转写 → 语义嵌入。执行方案铁律 8 要求逐版查证；
铁律 8/红线 1 要求新顶层依赖走 ADR + cargo deny。本 ADR 一次覆盖一批
「无聊依赖」，避免逐个走流程。

## 决策

| 依赖 | 版本 | 落点 | 用途 | 选型理由 |
|---|---|---|---|---|
| image | 0.25 | workspace.deps | 缩略图生成/编解码 | 纯 Rust 事实标准；0.25 为当前稳定线 |
| kamadak-exif | 0.6 | workspace.deps | EXIF 抽取 | 纯 Rust、只读解析、无 unsafe 声明 |
| ort | =2.0.0-rc.13 | ai-ocr feature | ONNX 推理底座（OCR） | 官方 onnxruntime 绑定；调研方案 §8「7.0/2.0rc」口径 |
| fastembed | =7.0.1 | ai-embed feature | BGE-M3/CLIP 嵌入 | 调研方案 §8 既定选型（ort 底） |
| whisper-rs | =0.16.0 | ai-transcribe feature | whisper.cpp 绑定（转写） | 调研方案 §5.12 既定选型 |

1. **feature 门控（默认全关）**：`ai-ocr` / `ai-embed` / `ai-transcribe`
   三个 feature 分别承载 ort / fastembed / whisper-rs。默认构建不编译
   任何推理栈——workspace 其余 crate 与 CI 不背负 C++/cmake 构建链与
   数百 MB 二进制（SPEC 风险项「编译与体积」的裁定落地）；
2. **rc/绑定版本精确锁定（`=`）**：ort 处于 2.0 RC 期（onnxruntime 1.28
   底），fastembed/whisper-rs 为绑定型 crate（上游 C/C++ 版本强耦合），
   统一精确锁避免 semver 兼容声明不实的传递漂移；上游出正式版后另开
   ADR 跟进；
3. **隐私红线**：三栈均为本地推理，管线代码禁止任何网络调用（SPEC
   裁定 6）；模型文件经清单（名称/版本/blake3/来源）校验后本地加载；
4. **构建环境要求**（feature 开启时）：ort 默认下载预编译 onnxruntime
   二进制；whisper-rs 需 cmake + 系统 C/C++ 编译链——登记为本机开发
   前置，CI 默认不开这三个 feature；
5. **我方 unsafe 面为零**：workspace `forbid(unsafe)` 不变；上述依赖
   内部（ort/whisper FFI 层）的 unsafe 属其自身边界，我方代码不得渗出。

## 后果

- deny.toml 基线以 T02 实测为准：新传递链如出现许可/advisory 缺口，
  按 ADR-0016 先例在同一 ADR 族登记豁免（无漏洞 + 无升级路径才豁免）；
- **T02 实测登记（2026-09-22）**：licenses +NCSA（libfuzzer-sys，经
  image→ravif→rav1e AVIF 路径；OSI/FSF 认可）；bans 修正 partisync-ai
  path 依赖补 version（wildcards=deny 基线）；advisories 全绿（新链
  无未豁免漏洞条目）；
- 默认构建（无 feature）体积与编译时间不受影响；三 feature 开启属
  端侧发布配置，归 M4 后续任务实测体积与吞吐（T05）；
- candle 备选路径保留：若 ort/fastembed 在 Metal 后端实测不可用
  （T05），以修订本 ADR 方式切换，不影响 SPEC 契约层（Stage trait
  隔离依赖类型，SPEC 契约 §2）。

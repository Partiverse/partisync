title: C2PA 内容真实性标准
filename: c2pa_overview.md
tags: [c2pa, content-authenticity, provenance, manifest, crypto]
updated_ns: 1726348800000000000

# C2PA 内容真实性标准

## 背景

C2PA（Coalition for Content Provenance and Authenticity）—— Adobe / Microsoft /
BBC / Intel 等联合的内容溯源开放标准。应对 deepfake 与 AI 生成内容溯源。

## 核心概念

- **Manifest**：嵌入资产内的元数据，含断言 + 签名
- **断言（assertion）**：c2pa.actions / c2pa.hash / 自定义
- **Ingredient**：被引用的上游资产（用于追溯链）
- **签名**：椭圆曲线（ES256/RSAPS256/Ed25519）

## 工作流

1. 拍摄/生成工具在导出时嵌入 manifest
2. 接收方解析 manifest 验证签名
3. 信任列表（TAU）判定 signer 是否可信

## PartiSync 集成（M4-WP05）

- **摄取校验**：Sidecar 第六阶段（c2pa stage）
- **保留**：manifest store report JSON 落 `content.c2pa` 列
- **离线确定性**：`verify_trust=false`（信任链依赖信任列表获取）
- **四态**：valid / invalid / absent / unsupported

## 已知坑位

- 测试证书 EKU 必须 `1.3.6.1.4.1.62558.2.1`（C2PA Signing）
- manifest 须含 `c2pa.actions` 首动作 created/opened
- JPEG 布局 = JUMBF 盒在前 + 扫描数据贴 EOI
- 篡改翻转位 = len-100（扫描数据区）

详见 `docs/specs/M4-WP05.md` 与 `docs/reports/bench/m4-wp05-kpi.md`。
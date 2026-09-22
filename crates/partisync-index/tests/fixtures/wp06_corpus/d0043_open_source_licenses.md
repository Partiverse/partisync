title: 开源许可证对比
filename: open_source_licenses_comparison.md
tags: [open-source, license, mit, apache, gpl, legal]
updated_ns: 1704067200000000000

# 开源许可证对比

## 主流许可证

| 许可证 | 商业可用 | 修改要求 | 分发要求 |
|---|---|---|---|
| MIT | ✅ | 保留版权 | 保留版权 |
| Apache-2.0 | ✅ | 保留版权 + 修改声明 | 专利授权 |
| BSD-3-Clause | ✅ | 保留版权 + 非 endorsement | 保留版权 |
| GPL-3.0 | ⚠️ | 衍生作品必须 GPL | 同 |
| AGPL-3.0 | ❌（网络服务也触发） | 同 GPL | 同 |
| MPL-2.0 | ✅ | 修改文件必须 MPL | 保留版权 |

## 选择指南

### MIT / BSD

- 简单 + 宽松
- 适合：库、工具

### Apache-2.0

- 包含专利授权
- 适合：企业级库

### GPL

- Copyleft
- 适合：保护衍生作品开放

### AGPL

- 网络服务也触发开源
- 适合：避免云厂商白嫖

### MPL

- 文件级 copyleft
- 适合：可与专有代码集成

## PartiSync 选择

- 默认 MIT（最宽松）
- 数据库/连接器代码可 Apache-2.0
- **禁用 GPL/AGPL**（与商业化冲突）

## 兼容性

- MIT + Apache → 可
- MIT + GPL → 可
- GPL + Apache → 仅 GPL
- GPL + AGPL → 仅 AGPL

## 实战注意

- 依赖 license 检查：`cargo deny check licenses`
- 双协议（dual licensing）时选 MIT
- 添加依赖时白名单：`MIT, Apache-2.0, BSD-3-Clause, ISC, Zlib, MPL-2.0, CC0-1.0`

## 贡献者协议

- DCO（Signed-off-by）
- CLA（Contributor License Agreement）
- 详见各项目 CONTRIBUTING.md

## 资源

- choosealicense.com
- OSI（Open Source Initiative）
- SPDX license list
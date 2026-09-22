title: Obsidian 笔记工作流
filename: obsidian_workflow.md
tags: [obsidian, notes, pkm, knowledge-management, productivity]
updated_ns: 1704067200000000000

# Obsidian 笔记工作流

## 核心理念

- 笔记 = 思维的建筑材料
- 双向链接 = 思维的网络
- 本地优先 + Markdown = 数据主权

## 文件夹结构

```
vault/
├── 0_inbox/        # 临时收集
├── 1_daily/        # 每日笔记（journal）
├── 2_projects/     # 项目笔记
├── 3_areas/        # 长期关注领域
├── 4_resources/    # 参考资料
├── 5_archive/      # 归档
└── _templates/     # 模板
```

## 模板

### Daily Note

```yaml
---
date: {{date}}
weather:
tags: [daily]
---
# {{date}}

## 今天
- 

## 想
- 

## 学习
- 

## 感恩
- 
```

## 关键插件

- **Dataview**: SQL-like 查询
- **Templater**: 模板自动化
- **Calendar**: 日历视图
- **Mind Map**: 思维导图
- **Excalidraw**: 手绘

## 与 PartiSync 集成

- Vault 是 M4 asset_organize 管理的天然场景
- 同步策略：Vault 文件夹走 iroh 跨设备
- 检索：asset_search 直接搜 Vault 全文

## 备份

- iCloud（自动）
- NAS（每周）
- Git（仅 .md）
title: Markdown 写作风格
filename: markdown_writing_style_guide.md
tags: [markdown, writing, documentation, style, technical-writing]
updated_ns: 1704067200000000000

# Markdown 写作风格

## 标题层级

- H1 = 文章标题（每个文档一个）
- H2 = 主要章节
- H3 = 子章节
- H4 = 极少用（结构太深）
- H5+ = 不用（考虑重写）

## 行文

- 短句优先（一句一个意思）
- 段落首句即论点
- 列表优于长段落
- 表格优于多列列表

## 链接

- 有意义：「详见 WP02」
- 无意义：❌「点这里」「这里」

## 代码

- 行内：`code`
- 块：
  ```rust
  let x = 1;
  ```
- 指定语言（高亮）

## 强调

- `*italic*` 偶尔用
- `**bold**` 极少用
- 不用下划线

## 引用

> 关键观点
> 用 blockquote

## 列表

- 无序：`-`
- 有序：`1.` `2.`
- 任务：`- [ ]` `- [x]`

## 元数据

```markdown
---
title: ...
tags: [...]
updated: ...
---
```

## 文档类型

### ADR（架构决策记录）

- 状态
- 背景
- 决策
- 备选
- 影响

### SPEC（规格）

- 动机
- 现状
- 契约
- 验收
- 任务分解

### Tutorial

- 目标
- 前置条件
- 步骤
- 下一步

### Reference

- 完整 API
- 参数表
- 示例

## PartiSync 文档

- ADR: `docs/adr/`
- SPEC: `docs/specs/`
- 报告: `docs/reports/`
- 审查: `docs/reviews/`
- 安全: `docs/security/`
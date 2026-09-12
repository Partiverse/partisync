# 盘查记录（生产构建 http://127.0.0.1:4200）

## 测试方法
- 点击/键盘每个交互组件
- console 错误 + DOM 状态变化

## 发现问题（截至表单区）

| # | 组件 | 现象 |
|---|------|------|
| 1 | **Command 内联 demo** | 默认 listbox 始终 [expanded] 显示，应该只显示 input，点击才出 listbox |
| 2 | (待继续) | |

| # | 组件 | 现象 |
|---|------|------|
| 1 | Command 内联 demo | 默认 listbox 始终展开，应点击 input 才出 |
| 2 | Recharts | 控制台报 Production error #31（"width(-1) and height(-1)" + "Legend can't be used as JSX component"），图表渲染失败/部分元素不显示 |
| 3 | Tabs | "账户" 按钮 [active] 但内容区为空段落（页面里两个 Tabs 的 tabpanel 都显示空 paragraph） |
| 4 | Sonner 触发 | 需在生产构建实测 |

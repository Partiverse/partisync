title: Tauri 桌面应用评估
filename: tauri_desktop_evaluation.md
tags: [tauri, desktop, rust, frontend, webview]
updated_ns: 1715817600000000000

# Tauri 桌面应用评估

## 与 Electron 对比

| 维度 | Tauri | Electron |
|---|---|---|
| 运行时 | 系统 WebView | Chromium |
| 安装包 | 5-15 MB | 100+ MB |
| 内存 | 30-80 MB | 200+ MB |
| 启动 | <500ms | 1-2s |
| 系统集成 | Rust + Plugin | 通过 Node 模块 |

## 架构

- 前端：任意 web 框架（React/Vue/Svelte）
- 后端：Rust
- IPC：`invoke` + `listen`
- 安全：默认 CSP 严格 + 自定义协议

## PartiSync 桌面壳（M6）

- Tauri 2.x
- 前端：SolidJS（轻量）
- 后端：PartiSync CLI 内嵌
- 系统集成：文件系统 + 通知 + 系统托盘

## 优势

- 复用 CLI 子命令
- Rust IPC 类型安全（vs Electron any-类型）
- 安装包小 5-10x

## 风险

- WebView 跨平台差异（macOS WKWebView / Windows WebView2 / Linux webkitgtk）
- 移动端支持还在 beta
- 生态比 Electron 年轻

## 决策

M6 启动时优先 Tauri，Electron 仅作 fallback。
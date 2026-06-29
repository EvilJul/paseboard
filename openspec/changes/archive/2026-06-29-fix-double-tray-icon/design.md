## Context

macOS 上 PaseBoard 启动后系统托盘出现两个相同图标。两个图标来自不同创建路径：

1. `tauri.conf.json` 中的 `app.trayIcon` 配置块 — Tauri 框架自动根据配置创建图标
2. `src-tauri/src/main.rs` 中的 `TrayIconBuilder::new().build()` — 代码显式创建第二个图标

两个图标的行为完全一样（同一个菜单），只是数量重复。

## Goals / Non-Goals

**Goals:**
- 启动后系统托盘只显示一个图标

**Non-Goals:**
- 不改变图标行为、菜单结构或交互方式

## Decisions

- **删除 `tauri.conf.json` 的 `trayIcon` 配置**，保留 `TrayIconBuilder` 代码方式。
  - 理由：`TrayIconBuilder` 提供了更灵活的运行时控制（动态菜单更新），删除配置块不影响任何功能。
  - 备选方案（放弃）：删除 `TrayIconBuilder` 改用纯配置声明——丧失动态控制能力。

## Risks / Trade-offs

- [低风险] 如果未来某人重新在 tauri.conf.json 中添加 trayIcon 配置，双图标问题会复现
  - 缓解措施：代码注释标注此问题

## Why

PaseBoard 在 macOS 上启动后显示 Dock 图标，但应用主要作为系统托盘工具运行。Dock 图标不提供额外功能，反而占用 Dock 空间，关闭窗口时应用隐藏到托盘但 Dock 图标仍残留。

## What Changes

- **新增** `#[cfg(target_os = "macos")]` 条件下调用 `set_activation_policy(Accessory)`，隐藏 macOS Dock 图标
- 无跨平台影响，Windows/Linux 构建不变

## Capabilities

### New Capabilities
- `dock-icon`: 管理 macOS Dock 图标显示状态

### Modified Capabilities

- (无)

## Impact

- `src-tauri/src/main.rs`: 在 `setup` 闭包中添加一行 cfg 条件调用
- 无新依赖，无 Schema 变更

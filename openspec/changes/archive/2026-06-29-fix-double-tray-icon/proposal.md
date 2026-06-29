## Why

PaseBoard 在 macOS 上启动后系统托盘出现两个相同图标。根因是 `tauri.conf.json` 中的 `trayIcon` 配置块和 `main.rs` 中的 `TrayIconBuilder` 各创建一个图标实例。纯配置问题，只需删除 `tauri.conf.json` 中的重复配置。

## What Changes

- **删除** `src-tauri/tauri.conf.json` 中的 `trayIcon` 配置块，只保留代码中的 `TrayIconBuilder`
- 无运行时逻辑变更，无 Rust 代码修改

## Capabilities

### New Capabilities

- `tray-icon-config`: 确保系统托盘图标在 tauri.conf.json 中只声明一次

### Modified Capabilities

- (无)

## Impact

- 只修改 `src-tauri/tauri.conf.json` 一个文件
- 向后兼容：删除配置后 `TrayIconBuilder` 正常工作

## Why

用户目前无法清空历史记录，即使手动删除 `~/.paseboard/history.db` 也无法通过 UI 操作。需要一个「清空全部历史」功能，让用户可以一键清除所有同步记录。

## What Changes

- **新增** `HistoryStorage::clear_all()` 公有方法，删除所有历史记录
- **新增** 存储层清空通道 `storage_clear_tx`，支持跨线程安全清空
- **新增** `clear_history` IPC 命令，前端通过 `invoke('clear_history')` 调用
- **新增** 前端「清空全部历史」按钮 + 确认弹窗
- 无第三方依赖变更，无数据库 Schema 变更

## Capabilities

### New Capabilities
- `history-clear`: 一键清空所有粘贴板同步历史记录

### Modified Capabilities

- (无)

## Impact

- `src-tauri/src/clipboard/storage.rs`: 新增 `clear_all()` 方法
- `src-tauri/src/app.rs`: 新增 `StorageClear` 通道 + `handle_storage_requests` 分支
- `src-tauri/src/main.rs`: 新增 `clear_history` IPC 命令 + `invoke_handler` 注册
- `ui/index.html`: 历史记录标签页添加「清空全部」按钮 + 确认弹窗

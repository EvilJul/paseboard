## Context

PaseBoard 将粘贴板同步历史存储在 SQLCipher 加密的 SQLite 数据库中（`~/.paseboard/history.db`）。存储层在独立线程中运行，通过三个 mpsc 通道与主应用通信：
- `storage_tx`: 插入请求（`StorageRequest`）
- `storage_query_tx`: 查询请求（`StorageQuery`，带 oneshot 回复）
- 无清空通道

前端通过 `invoke('get_history')` IPC 命令查询历史，`IpcHandles` 持有 `storage_query_tx` 的发送端。

## Goals / Non-Goals

**Goals:**
- 用户可通过 UI 一键清空所有历史记录
- 清空操作有二次确认，防止误操作
- 清空后历史记录标签页立即显示空状态

**Non-Goals:**
- 不修改数据库 Schema
- 不提供撤销功能（清空即永久删除）
- 不修改存储容量管理逻辑

## Decisions

**D1: 新增 `storage_clear_tx` 通道**
   - 新增 `StorageClear` 类型（带 oneshot 回复通道），通过独立 mpsc 通道发送
   - `handle_storage_requests` 增加 `storage_clear_rx` 分支
   - 备选方案 1：复用 `StorageRequest` 变体 → 侵入插入通道，破坏单一职责
   - 备选方案 2：直接通过 `storage_tx` 发送特殊请求 → 需要枚举变体，耦合不同类型操作

**D2: `clear_all()` 使用 `DELETE FROM` 而非 `DROP TABLE`**
   - `DELETE FROM clipboard_history` 保留表结构和索引，性能可接受（最多 1000 条）
   - 备选方案：`DROP TABLE` + `CREATE TABLE` → 多余，且需重建索引

**D3: 前端使用原生 `confirm()` 弹窗**
   - 简单可靠，无需引入对话框组件
   - 不影响现有 UI 结构

**D4: `IpcHandles` 新增 `storage_clear_tx` 字段**
   - 与 `storage_query_tx` 同模式，保持架构一致性

## Risks / Trade-offs

- [低风险] 清空时如果有正在进行的插入操作 → `handle_storage_requests` 在单线程中顺序处理，不会并发冲突
- [低风险] 大量历史记录时 `DELETE` 可能短暂阻塞存储线程 → 最多 1000 条，毫秒级完成

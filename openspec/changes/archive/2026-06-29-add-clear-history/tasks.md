## 1. Backend: Storage Layer

- [x] 1.1 Promote `clear()` in `storage.rs` from `#[cfg(test)]` to public `clear_all()`, remove test gate
- [x] 1.2 Verify `clear_all()` compiles and tests pass

## 2. Backend: App Coordination

- [x] 2.1 Add `StorageClear` struct with oneshot reply channel in `app.rs`
- [x] 2.2 Add `storage_clear_tx/rx` channel in `App::new()`, pass to `handle_storage_requests`
- [x] 2.3 Add clear handling branch in `handle_storage_requests` loop
- [x] 2.4 Add `storage_clear_tx` field to `IpcHandles`

## 3. Backend: IPC Command

- [x] 3.1 Add `clear_history` IPC command in `main.rs`
- [x] 3.2 Register `clear_history` in `invoke_handler`

## 4. Frontend: UI

- [x] 4.1 Add "清空全部历史" button in the history tab header
- [x] 4.2 Add confirmation dialog using `confirm()` before invoking
- [x] 4.3 Wire button to `invoke('clear_history')` and refresh history on success

## 5. Verification

- [x] 5.1 Code review: all references consistent, `clear_history`, `clear_all`, `StorageClear`, `storage_clear_tx` in correct places
- [x] 5.2 `cargo check` passed

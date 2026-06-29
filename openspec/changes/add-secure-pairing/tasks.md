## 1. Storage: Pairing Tables

- [x] 1.1 Add `paired_devices` table (device_id, device_name, paired_at) in `storage.rs`
- [x] 1.2 Add `pairing_cooldown` table (device_id, cooldown_until) in `storage.rs`
- [x] 1.3 Add storage methods: `is_paired`, `list_paired_devices`, `add_paired_device`, `remove_paired_device`
- [x] 1.4 Add storage methods: `is_in_cooldown`, `set_cooldown`

## 2. App: Pairing Storage Channel

- [x] 2.1 Add `PairingOp` enum + mpsc channel in `app.rs`
- [x] 2.2 Add pairing handling branch in `handle_storage_requests`
- [x] 2.3 Add `IpcHandles` field for pairing channel

## 3. Network: Message Types

- [x] 3.1 Add `PairingRequest` variant to `MessageType` (device_id, device_name, device_pk_fingerprint)
- [x] 3.2 Add `PairingResponse` variant to `MessageType` (accepted: bool, reason: Option<String>)
- [x] 3.3 Add construction + helper methods (`is_pairing`, `new_pairing_request`, `new_pairing_response`)

## 4. Connection Sequence

- [x] 4.1 After `WebSocketClient::connect()` succeeds in `connect_to_device`: send `PairingRequest` via client
- [x] 4.2 In `connect_to_device` receiver loop: add `PairingResponse` + `PairingRequest` routing; pairing check before clipboard processing

## 5. Server Message Routing

- [x] 5.1 In `handle_incoming_messages_task`: add `PairingRequest` + `PairingResponse` routing; pairing check before clipboard processing
- [x] 5.2 WebSocket server already forwards non-heartbeat messages via `message_tx` — pairing messages are handled automatically

## 6. IPC Commands

- [x] 6.1 Add `get_paired_devices` IPC command
- [x] 6.2 Add `remove_pairing` IPC command
- [x] 6.3 Register new commands in `invoke_handler`

## 7. Frontend: Pairing UI

- [x] 7.1 Add pairing status column to device list (load paired data, render badge)
- [x] 7.2 Add unpair button with confirmation dialog

## 8. Verification

- [x] 8.1 `cargo check` passes
- [x] 8.2 `cargo test --lib` passes

## Out of Scope (Follow-up)

- Pairing confirmation modal/notification — current implementation auto-accepts (zero-config). Add when explicit pairing approval is needed.
- `respond_pairing` IPC command — not needed while auto-accept is enabled.
- Pairing cooldown enforcement — cooldown storage exists but is not enforced yet. Add when explicit pairing rejection is implemented.

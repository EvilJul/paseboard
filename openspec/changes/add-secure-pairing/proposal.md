## Why

PaseBoard 目前任何兼容设备都可以自动连接并接收粘贴板内容，没有任何身份验证。在多设备环境下，需要设备配对机制确保只有用户授权的设备可以接收粘贴板同步内容。

## What Changes

- **新增** `PairingRequest` / `PairingResponse` WebSocket 消息类型
- **新增** `paired_devices` 数据库表，持久化已配对设备列表
- **新增** 30 分钟配对冷却（D2），防止频繁配对请求
- **新增** 连接序列改造（D9）：连接建立后第一条消息协商配对状态
- **新增** 消息路由分支（D10）：两个消息接收循环处理配对消息
- **新增** IPC 命令：`get_paired_devices`, `respond_pairing`, `remove_pairing`, `reset_pairing_cooldown`
- **新增** 前端配对确认弹窗、设备配对状态显示
- **修改** mDNS TXT records 添加配对状态字段
- 向后兼容：未配对设备和旧版本设备收到 `PairingResponse { accepted: false }` 后优雅降级

## Capabilities

### New Capabilities
- `device-pairing`: 设备配对生命周期管理（请求、确认、拒绝、取消配对、冷却）

### Modified Capabilities

- `tray-icon-config`: 无需求变更
- `history-clear`: 无需求变更

## Impact

- `message.rs`: 新增 `PairingRequest` + `PairingResponse` 枚举变体 + 构造方法 + 判断方法
- `storage.rs`: 新增 `paired_devices` 表 + `PairingStorage` 方法（`is_paired`, `add_pairing`, `remove_pairing`, `is_in_cooldown`, `set_cooldown`）
- `websocket_client.rs`: 初始心跳替换为配对协商序列
- `websocket_server.rs`: 接收任务新增配对消息分支
- `app.rs`: `handle_incoming_messages_task` + `connect_to_device` receiver 新增配对路由
- `main.rs`: 新增 4 个 IPC 命令
- `mdns.rs`: TXT records 新增配对状态
- `index.html`: 设备列表显示配对状态 + 配对确认弹窗
- 无新第三方依赖

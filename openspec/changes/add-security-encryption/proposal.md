## Why

CSO 安全审计发现 WebSocket 明文传输粘贴板内容、设备身份无验证、SQLite 历史未加密三项架构级安全问题。需在通信层、身份层、存储层增加加密保护，确保粘贴板内容在传输和存储过程中不被未授权方获取。

## What Changes

- 设备首次运行自动生成 Ed25519 密钥对，持久化到磁盘
- mDNS/UDP 发现广播增加设备公钥信息
- WebSocket 连接建立时进行 X25519 ECDH 密钥交换，派生 AES-256-GCM 对称密钥
- 所有消息载荷使用 AES-GCM 加密传输
- 设备身份验证与加密合并（ECDH 即隐含身份验证）
- SQLite 数据库使用 SQLCipher 加密，密钥由平台密钥链存储
- 旧版本设备无法与新版本通信（向后不兼容）

## Capabilities

### New Capabilities

- `device-identity`: Ed25519 密钥对生成、持久化、公钥广播
- `encrypted-transport`: WebSocket 消息 AES-GCM 加密/解密
- `encrypted-storage`: SQLCipher + 平台密钥链加密的粘贴板历史存储

### Modified Capabilities

- （无现有 spec 需修改）

## Impact

- 新增 crate：`x25519-dalek`, `aes-gcm`, `rand`, `keyring`, `rusqlite` (sqlcipher feature)
- 修改文件：`config.rs`, `mdns.rs`, `websocket_client.rs`, `websocket_server.rs`, `message.rs`, `message.rs` (payload 加密层), `storage.rs`, `app.rs`
- 通信协议变更：消息格式增加加密载荷字段
- 向后兼容：旧版本设备无法与新版本配对通信

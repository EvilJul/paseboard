## ADDED Requirements

### Requirement: WebSocket 连接建立
系统 SHALL 在设备间建立双向 WebSocket 连接，用于实时消息传输。

#### Scenario: 成功建立连接
- **WHEN** 两台设备通过 mDNS 发现彼此
- **THEN** 客户端向服务端发起 WebSocket 连接并完成握手

#### Scenario: 连接失败重试
- **WHEN** WebSocket 连接失败
- **THEN** 系统使用指数退避策略重试，最多重试 3 次

### Requirement: 消息实时推送
系统 SHALL 在检测到粘贴板变化后立即通过 WebSocket 推送到所有已连接设备。

#### Scenario: 粘贴板变化触发推送
- **WHEN** 本地粘贴板内容发生变化（来自用户复制操作）
- **THEN** 系统在 1 秒内将内容推送到所有在线设备

#### Scenario: 并发推送到多台设备
- **WHEN** 存在 5 台已连接设备
- **THEN** 系统并发发送消息到所有设备，总延迟不超过 1 秒

### Requirement: 消息格式规范
消息 SHALL 使用 JSON 格式，包含 UUID、内容、设备 ID、时间戳字段。

#### Scenario: 消息包含必需字段
- **WHEN** 系统生成同步消息
- **THEN** 消息包含 `uuid`（唯一标识）、`content`（粘贴板内容）、`device_id`（来源设备）、`timestamp`（Unix 时间戳）

#### Scenario: 消息序列化与反序列化
- **WHEN** 发送端序列化消息为 JSON
- **THEN** 接收端能够正确反序列化并提取所有字段

### Requirement: 心跳保活机制
系统 SHALL 每 30 秒发送一次心跳消息，检测连接是否存活。

#### Scenario: 定期发送心跳
- **WHEN** WebSocket 连接建立后
- **THEN** 系统每 30 秒向对端发送心跳消息

#### Scenario: 心跳超时断开连接
- **WHEN** 超过 60 秒未收到对端心跳响应
- **THEN** 系统关闭该连接并标记设备为离线

### Requirement: 断线自动重连
系统 SHALL 在 WebSocket 连接断开后自动尝试重连。

#### Scenario: 网络中断后重连
- **WHEN** WebSocket 连接因网络问题断开
- **THEN** 系统等待 2 秒后发起第一次重连

#### Scenario: 指数退避重连策略
- **WHEN** 第一次重连失败
- **THEN** 系统依次等待 2 秒、4 秒、8 秒后重试，最多重试 3 次

### Requirement: 内容大小限制
系统 SHALL 拒绝发送超过 10MB 的粘贴板内容，并在 UI 显示警告。

#### Scenario: 内容超过限制时拒绝发送
- **WHEN** 粘贴板内容大小超过 10MB
- **THEN** 系统不发送该内容，并在 UI 显示"内容超过 10MB，不同步"警告

#### Scenario: 内容未超过限制时正常发送
- **WHEN** 粘贴板内容大小为 5MB
- **THEN** 系统正常序列化并发送该内容

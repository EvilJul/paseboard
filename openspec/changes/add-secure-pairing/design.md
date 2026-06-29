## Context

PaseBoard 当前无设备身份验证——任何兼容设备发现后自动连接并接收粘贴板内容。F4 引入用户确认的配对机制，确保只有授权设备参与同步。

已有基础：
- WebSocket 加密传输（AES-256-GCM + ECDH）
- Ed25519 设备身份密钥
- 消息协议支持扩展（`#[serde(tag = "type")]` 枚举）
- SQLite 存储（SQLCipher 加密）

## Goals / Non-Goals

**Goals:**
- 新设备首次连接时提示用户确认
- 已配对设备自动连接（无需再次确认）
- 30 分钟冷却期防止配对骚扰
- 设备列表显示配对状态
- 用户可管理（取消配对）已配对设备
- 向后兼容：旧版本显示为"未配对"但不影响已有连接

**Non-Goals:**
- 不引入配对码/QR 码（简化交互）
- 不改变加密传输方式
- 不修改数据库 Schema 版本号（新表创建用 IF NOT EXISTS）

## Decisions

**D1: 首次连接配对序列替换初始心跳**
   - 连接建立后第一条消息改为 `PairingRequest`（含设备 ID、设备名、公钥指纹）
   - 服务端查询 `paired_devices` 表决定回复 `accepted: true/false`
   - 如果 accepted=true，后续正常收发粘贴板消息
   - 如果 accepted=false，客户端进入等待配对状态（前端触发）

**D2: 配对状态持久化到 SQLite**
   - 新建 `paired_devices` 表：`(id INTEGER PK, device_id TEXT UNIQUE, device_name TEXT, paired_at INTEGER)`
   - 新建 `pairing_cooldown` 表：`(device_id TEXT PK, cooldown_until INTEGER)`
   - 所有操作在 `handle_storage_requests` 同一线程中处理（通过新通道）
   - 理由：D2 已决策，Cooldown 必须持久化

**D3: 消息路由 - 两个接收循环都新增配对分支**
   - Server 端：`handle_incoming_messages_task` 新增 `msg.is_pairing()` 分支
   - Client 端：`connect_to_device` 的 receiver 新增 `msg.is_pairing()` 分支
   - 遵循 D10 决策

**D4: 配对状态通过 `IpcHandles` 共享**
   - `IpcHandles` 新增 `paired_devices` Arc\<RwLock\<HashSet\<String\>\>\>（内存缓存）
   - 启动时从 `paired_devices` 表加载到缓存
   - IPC 命令直接读缓存（避免跨线程查询延迟）

**D5: 配对请求通过 `broadcast` 而非点对点**
   - 当前消息通过 WebSocket Server broadcast 发送
   - PairingRequest 同理：服务端广播到所有客户端，配对逻辑由应用层处理
   - 简化架构：不需要为配对建立单独连接

## Risks / Trade-offs

- [低风险] 旧版本设备不识别 PairingRequest → 保持向后兼容，对方忽略该消息，不影响粘贴板同步
- [低风险] 配对请求可能丢失 → 设备每 5 秒重试发现流程，重连时会重新发起配对
- [中等风险] 同时发起配对请求 → 两端各等对方回复，形成死锁 → 缓解：设备 ID 较小的一方主动回复 accept

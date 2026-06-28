## Context

PaseBoard 当前未对粘贴板内容做任何加密保护。WebSocket 消息以 JSON 明文传输，SQLite 数据库文件无加密，设备身份仅靠自报告的 device_id 字符串。CSO 安全审计发现此三项为架构级风险。

## Goals / Non-Goals

**Goals:**
- 所有 WebSocket 消息使用 AES-256-GCM 加密传输
- 设备身份通过 Ed25519 公钥指纹验证（与加密合并）
- 粘贴板历史数据库使用 SQLCipher + 平台密钥链加密
- 首次运行自动生成密钥对，无需用户配置

**Non-Goals:**
- 不引入 CA/TLS 证书体系
- 不改变设备发现机制（mDNS/UDP 仍用于发现）
- 不增加用户配置或注册流程
- 不实现密钥轮换或吊销（留待后续）

## Decisions

### 1. 密钥协商：X25519 ECDH + AES-256-GCM
- 设备首次运行时用 `ed25519-dalek` 生成 Ed25519 密钥对
- 公钥通过 mDNS TXT 记录和 UDP 广播共享
- WebSocket 连接时使用 X25519（Curve25519 ECDH）派生共享密钥
- 共享密钥通过 HKDF-SHA256 派生为 AES-256-GCM 对称密钥
- 每个连接独立派生密钥（前向安全）

### 2. 消息加密格式
```
EncryptedMessage {
    nonce: [u8; 12],      // AES-GCM 随机 nonce
    ciphertext: Vec<u8>,   // 加密后的消息载荷
    public_key: [u8; 32],  // 发送方的临时公钥（用于 ECDH 计算）
}
```
- 每次发送生成随机 nonce（12 字节）
- 接收方用自身私钥 + 消息中的发送方公钥计算共享密钥
- 解密失败 → 断开连接（身份验证失败）

### 3. 存储加密：SQLCipher + keyring
- 使用 `rusqlite` 的 `sqlcipher` feature 替代普通 SQLite
- 主密钥由 `keyring` crate 从平台密钥链获取（macOS Keychain / Linux Secret Service / Windows DPAPI）
- 密钥格式：32 字节随机密钥，base64 编码后存入密钥链
- 应用启动时从密钥链读取密钥，打开加密数据库

### 4. 向后兼容
- v0.2.0 将无法与 v0.1.x 设备通信
- mDNS 广播中增加 `crypto_version` 字段标识加密版本
- 发现旧版本设备时显示"不兼容"状态而非建立连接

## Risks / Trade-offs

- [加密性能开销] → AES-GCM 在硬件加速下开销可忽略（<1ms 每条消息）
- [密钥丢失] → 无法恢复历史记录。首次运行时提示用户备份密钥（未来特性）
- [向后不兼容] → 所有设备需升级到 v0.2.0 才能通信。这是必要的一步
- [keyring 跨平台差异] → macOS/Linux/Windows 密钥链 API 不同，keyring crate 已抽象
- [平台密钥链不可用] → 降级到基于文件 + 用户口令的加密方案

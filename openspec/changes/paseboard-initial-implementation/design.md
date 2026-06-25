## Context

PaseBoard 是一个跨平台局域网粘贴板同步工具，解决多设备工作场景下的内容传输痛点。当前状态是全新项目，无现有代码库。

**技术背景：**
- 桌面框架：Tauri v1（轻量级、跨平台）
- 后端语言：Rust 1.70+（内存安全、并发性能）
- 异步运行时：Tokio（通过 Tauri 依赖）
- 网络协议：mDNS（设备发现）+ WebSocket（实时通信）

**约束条件：**
- 目标平台：Windows、macOS、Linux
- 网络环境：仅局域网（无公网穿透）
- 设备上限：建议不超过 10 台（全连接网络 O(N²)）
- 内容类型：纯文本（MVP 阶段）

## Goals / Non-Goals

**Goals:**
- 零配置：启动即自动发现局域网内其他设备
- 实时同步：粘贴板内容 1 秒内同步到其他设备
- 轻量级：内存占用 < 50MB，CPU 占用（空闲）< 1%
- 跨平台：统一代码库支持 Windows/macOS/Linux
- 历史记录：最近 1000 条粘贴板历史可查询

**Non-Goals:**
- 公网同步（不支持跨网段设备）
- 端到端加密（MVP 阶段明文传输）
- 图片/文件同步（仅支持纯文本）
- 移动端支持（桌面端优先）
- 云端备份

## Decisions

### Decision 1: 网络拓扑 - 全连接 P2P

**选择：** 全连接网络（每台设备与所有其他设备建立双向 WebSocket 连接）

**理由：**
- 适合典型场景（3-5 台设备）
- 无单点故障
- 延迟最低（直连，无中继）

**替代方案：**
- 星型拓扑（一台设备作为中继服务器）：增加单点故障风险
- 混合拓扑（部分设备中继）：复杂度高，MVP 阶段不适合

**权衡：**
- 优势：简单、可靠、低延迟
- 劣势：设备数 N 增加时连接数 O(N²)，建议上限 10 台

### Decision 2: 设备发现 - mDNS

**选择：** 使用 mDNS 协议进行设备发现，服务类型 `_paseboard._tcp.local`

**理由：**
- 标准协议，跨平台支持良好
- 无需中心服务器
- `mdns-sd` crate 成熟稳定

**替代方案：**
- 广播扫描（UDP broadcast）：某些网络环境禁用广播
- 手动配置 IP：用户体验差，违背"零配置"目标

**权衡：**
- 优势：自动化、跨平台
- 劣势：依赖 mDNS 服务（Windows 需 Bonjour，Linux 需 Avahi）

### Decision 3: 消息去重 - 双重保险

**选择：** UUID 去重 + 内容哈希去重

**理由：**
- 防止消息回环（设备 B 收到消息写入粘贴板后，监听器检测到变化不再推送）
- 双重保险：即使一种机制失效，另一种兜底

**实现：**
- UUID 去重：每条消息带唯一标识，接收端记录已处理 UUID
- 内容哈希去重：推送前计算内容 SHA256 哈希，相同内容跳过
- 来源标记：写入粘贴板时标记"来自网络"，监听器检测到该标记跳过推送

**权衡：**
- 优势：可靠防止回环
- 劣势：略增加计算开销（哈希计算）

### Decision 4: 历史存储 - SQLite

**选择：** 使用 SQLite 存储粘贴板历史，数据库文件 `~/.paseboard/history.db`

**理由：**
- 嵌入式数据库，无需独立进程
- 跨平台支持
- `rusqlite` crate 成熟

**Schema：**
```sql
CREATE TABLE clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    size INTEGER NOT NULL
);
CREATE INDEX idx_timestamp ON clipboard_history(timestamp DESC);
CREATE INDEX idx_content_hash ON clipboard_history(content_hash);
```

**替代方案：**
- 纯内存缓存：应用重启后丢失历史
- 文件存储（JSON）：查询性能差，无索引

**权衡：**
- 优势：持久化、查询快、索引支持
- 劣势：需要维护数据库文件

### Decision 5: 粘贴板监听 - 固定轮询

**选择：** 固定 500ms 轮询间隔

**理由：**
- 代码简单
- 延迟可接受（用户复制后 500ms 内同步）
- CPU 占用低（< 1%）

**替代方案：**
- 事件驱动（系统粘贴板变化事件）：跨平台支持不一致
- 自适应轮询间隔：增加复杂度，用户体验提升不明显

**权衡：**
- 优势：简单、可靠
- 劣势：理论最大延迟 500ms（实际用户感知不明显）

### Decision 6: 消息序列化 - JSON

**选择：** 使用 JSON 格式序列化消息

**理由：**
- 人类可读，便于调试
- `serde_json` 性能足够（< 1ms 序列化 1KB 内容）
- 跨语言兼容性好（未来可能支持其他客户端）

**消息格式：**
```json
{
  "uuid": "550e8400-e29b-41d4-a716-446655440000",
  "content": "Hello, world!",
  "device_id": "device-uuid",
  "timestamp": 1703520000
}
```

**替代方案：**
- Protobuf：性能更好但增加复杂度，MVP 阶段不需要
- MessagePack：二进制格式，调试困难

**权衡：**
- 优势：简单、可读、兼容性好
- 劣势：略大于二进制格式（实际影响不大，网络带宽充足）

### Decision 7: 错误处理 - 混合模式

**选择：** 核心模块自定义 Error enum（`thiserror`）+ 应用层 `anyhow::Result`

**理由：**
- 核心模块（`network/`, `clipboard/`）需要细粒度错误类型，便于匹配处理
- 应用层（`app.rs`, `main.rs`）追求简洁，使用 `anyhow` 简化错误传播

**实现：**
```rust
// 核心模块
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("连接失败: {0}")]
    ConnectionFailed(String),
    // ...
}

// 应用层
pub async fn run_app() -> anyhow::Result<()> {
    let network = init_network().await?;
    // ...
}
```

**权衡：**
- 优势：核心模块类型安全，应用层简洁
- 劣势：需要维护两套错误处理风格

### Decision 8: 依赖版本管理 - 精确锁定

**选择：** `Cargo.toml` 中所有依赖使用精确版本（如 `version = "1.35.0"`）

**理由：**
- 避免 `cargo update` 意外引入破坏性更新
- 确保所有开发者构建结果一致

**权衡：**
- 优势：构建可复现
- 劣势：需要手动检查依赖更新和安全补丁

## Risks / Trade-offs

### Risk 1: mDNS 依赖外部服务

**风险：** Windows 用户需安装 Bonjour，Linux 用户需启用 Avahi，否则设备发现失败

**缓解：**
- 在 README 中明确说明依赖要求
- 启动时检测 mDNS 服务是否可用，不可用时显示友好错误提示
- v0.2 考虑增加手动 IP 输入作为降级方案

### Risk 2: 全连接网络的扩展性

**风险：** 设备数超过 10 台时，连接数 O(N²) 导致资源消耗过高

**缓解：**
- 在 UI 中显示设备数警告（超过 10 台时）
- 文档中明确说明建议设备上限
- 未来版本可考虑混合拓扑或星型中继

### Risk 3: 明文传输的安全性

**风险：** 局域网内数据明文传输，可被同网络设备嗅探

**缓解：**
- 在 README 和首次启动时警告用户
- v0.2 增加可选的端到端加密
- 日志中不记录粘贴板内容原文

### Risk 4: 粘贴板轮询的 CPU 占用

**风险：** 500ms 轮询可能在低性能设备上占用较多 CPU

**缓解：**
- 实测 CPU 占用 < 1%（现代 CPU）
- 如果用户反馈 CPU 占用问题，v0.2 考虑自适应轮询间隔

### Risk 5: 大内容的性能影响

**风险：** 接近 10MB 的内容序列化和传输可能导致延迟 > 1秒

**缓解：**
- 在发送前检查大小，超过 10MB 拒绝发送并显示警告
- 序列化优化：序列化一次，并发发送给所有设备
- 未来版本可考虑压缩或流式传输

## Migration Plan

**初始部署：**
1. 用户下载安装包（Windows: .msi, macOS: .dmg, Linux: .deb/.rpm）
2. 首次启动时自动创建配置目录 `~/.paseboard/`
3. 生成设备 UUID 并保存到 `config.toml`
4. 初始化 SQLite 数据库 `history.db`

**无需迁移计划：** 这是全新项目，无现有用户

**回滚策略：**
- 桌面应用可直接卸载
- 数据库文件保留在 `~/.paseboard/`，用户可手动删除

## Open Questions

1. **设备重命名：** 用户是否需要手动修改设备名称？（当前使用计算机名称）
   - 建议：v0.1 使用计算机名称，v0.2 增加重命名功能

2. **历史记录搜索：** 是否需要全文搜索功能？
   - 建议：v0.1 不支持，v0.2 根据用户反馈决定

3. **端口冲突的备用范围：** 9527-9537 是否足够？
   - 建议：先实现该范围，后续根据用户反馈调整

4. **内容格式检测：** 是否需要检测内容类型（HTML vs 纯文本）？
   - 建议：v0.1 全部作为纯文本处理，v0.2 考虑格式保留

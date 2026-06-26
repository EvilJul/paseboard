# PaseBoard 架构文档

本文档描述 PaseBoard 的系统架构、模块设计和技术决策。

## 目录

- [架构总览](#架构总览)
- [分层设计](#分层设计)
- [核心模块](#核心模块)
- [数据流](#数据流)
- [技术选型](#技术选型)
- [目录结构](#目录结构)

## 架构总览

PaseBoard 采用分层架构设计，从下到上分为四层：

```
┌─────────────────────────────────────────┐
│         UI 层 (Tauri Frontend)          │  用户界面、事件处理
├─────────────────────────────────────────┤
│      应用层 (App Coordinator)           │  业务逻辑协调、状态管理
├─────────────────────────────────────────┤
│  功能层 (Network, Clipboard, Storage)   │  核心功能模块
├─────────────────────────────────────────┤
│    基础设施层 (mDNS, WebSocket, SQLite) │  底层协议和存储
└─────────────────────────────────────────┘
```

### 设计原则

1. **模块化**：每个模块职责单一，接口清晰
2. **异步优先**：所有 I/O 操作使用 Tokio 异步运行时
3. **错误透明**：核心模块使用类型化错误，应用层使用 anyhow 简化传播
4. **跨平台一致**：通过抽象层屏蔽平台差异

## 分层设计

### 1. UI 层

**职责：**
- 展示设备列表、历史记录
- 处理用户交互（点击、搜索、配置）
- 系统托盘集成

**技术栈：**
- Tauri WebView（HTML + CSS + JavaScript）
- 通过 Tauri IPC 与后端通信

**关键文件：**
- `ui/index.html` - 主界面
- `ui/styles.css` - 样式定义
- `ui/app.js` - 前端逻辑

### 2. 应用层

**职责：**
- 协调各功能模块
- 管理应用生命周期
- 处理 Tauri 命令调用
- 维护全局状态（已连接设备、当前配置）

**关键组件：**
- `App` 结构体：应用总控制器
- `AppState`：全局共享状态（使用 Arc<Mutex<T>>）
- Tauri 命令处理函数（`get_devices`, `get_history` 等）

**关键文件：**
- `src-tauri/src/app.rs` - 应用协调器
- `src-tauri/src/main.rs` - 程序入口

### 3. 功能层

#### 3.1 网络模块 (Network)

**职责：**
- 设备发现（mDNS 广播和监听）
- 建立和维护 WebSocket 连接
- 消息序列化/反序列化
- 连接管理（重连、超时、断线检测）

**关键类型：**
```rust
pub struct NetworkManager {
    device_id: String,
    device_name: String,
    mdns_service: MdnsService,
    connections: Arc<Mutex<HashMap<String, WsConnection>>>,
    message_tx: mpsc::Sender<NetworkMessage>,
}

pub struct NetworkMessage {
    pub uuid: String,
    pub content: String,
    pub device_id: String,
    pub timestamp: i64,
}
```

**关键文件：**
- `src-tauri/src/network/manager.rs` - 网络管理器
- `src-tauri/src/network/mdns.rs` - mDNS 服务
- `src-tauri/src/network/websocket.rs` - WebSocket 连接
- `src-tauri/src/network/protocol.rs` - 消息协议

#### 3.2 粘贴板模块 (Clipboard)

**职责：**
- 监听本地粘贴板变化（轮询）
- 读取和写入粘贴板内容
- 去重逻辑（避免消息回环）

**关键类型：**
```rust
pub struct ClipboardMonitor {
    clipboard: Clipboard,
    last_hash: String,
    poll_interval: Duration,
    is_network_write: Arc<AtomicBool>, // 标记是否来自网络写入
}

pub struct ClipboardEvent {
    pub content: String,
    pub hash: String,
    pub timestamp: i64,
}
```

**去重机制：**
1. **内容哈希去重**：记录上次内容的 SHA256 哈希，相同内容跳过
2. **来源标记**：网络消息写入粘贴板时设置标记，监听器检测到标记时跳过推送
3. **UUID 去重**：接收到的消息 UUID 记录在缓存中，避免重复处理

**关键文件：**
- `src-tauri/src/clipboard/monitor.rs` - 粘贴板监听器
- `src-tauri/src/clipboard/manager.rs` - 粘贴板管理器

#### 3.3 存储模块 (Storage)

**职责：**
- 粘贴板历史持久化
- 历史查询（按时间倒序、按内容搜索）
- 自动清理（保留最近 1000 条）

**数据库 Schema：**
```sql
CREATE TABLE clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    size INTEGER NOT NULL
);

CREATE INDEX idx_timestamp ON clipboard_history(timestamp DESC);
CREATE INDEX idx_content_hash ON clipboard_history(content_hash);
```

**关键类型：**
```rust
pub struct StorageManager {
    conn: Connection,
}

pub struct HistoryEntry {
    pub id: i64,
    pub content: String,
    pub device_id: String,
    pub device_name: String,
    pub timestamp: i64,
    pub size: i64,
}
```

**关键文件：**
- `src-tauri/src/storage/manager.rs` - 存储管理器
- `src-tauri/src/storage/models.rs` - 数据模型

### 4. 基础设施层

**mDNS (mdns-sd crate):**
- 服务类型：`_paseboard._tcp.local`
- 端口范围：9527-9537（动态选择可用端口）
- 服务属性：设备 ID、设备名称、版本号

**WebSocket (tokio-tungstenite):**
- 协议：WebSocket over TCP
- 消息格式：JSON
- 心跳机制：每 30 秒发送 ping 帧

**SQLite (rusqlite):**
- 数据库位置：`~/.paseboard/history.db`
- 连接模式：单写多读
- WAL 模式：提高并发性能

## 数据流

### 1. 设备发现流程

```
设备 A 启动
    │
    ├─> 注册 mDNS 服务 (_paseboard._tcp.local)
    │   (包含设备 ID、名称、WebSocket 端口)
    │
    └─> 监听 mDNS 服务
            │
            └─> 发现设备 B
                    │
                    └─> 建立 WebSocket 连接到设备 B
                            │
                            └─> 加入已连接设备列表
```

### 2. 粘贴板同步流程

```
用户在设备 A 复制内容
    │
    ├─> ClipboardMonitor 检测到变化 (500ms 轮询)
    │   │
    │   ├─> 计算内容哈希
    │   │
    │   └─> 检查去重 (哈希 != 上次哈希 && 非网络写入)
    │
    ├─> 生成 NetworkMessage
    │   - UUID: 唯一标识
    │   - Content: 粘贴板内容
    │   - DeviceID: 设备 A 的 ID
    │   - Timestamp: 当前时间戳
    │
    ├─> 序列化为 JSON
    │
    ├─> 通过 WebSocket 发送给所有连接的设备
    │   (并发发送，不等待响应)
    │
    └─> 保存到本地数据库
            │
            └─> StorageManager.save_entry()

设备 B 接收消息
    │
    ├─> 反序列化 JSON
    │
    ├─> 检查 UUID 去重 (未处理过该 UUID)
    │
    ├─> 写入本地粘贴板
    │   - 设置 is_network_write = true
    │   - ClipboardMonitor 检测到标记，跳过推送
    │
    └─> 保存到本地数据库
            │
            └─> StorageManager.save_entry()
```

### 3. 历史记录查询流程

```
用户打开历史记录界面
    │
    └─> Tauri 命令: get_history(limit, offset)
            │
            ├─> StorageManager.query_history()
            │   │
            │   └─> SQL: SELECT * FROM clipboard_history
            │           ORDER BY timestamp DESC
            │           LIMIT ? OFFSET ?
            │
            └─> 返回 HistoryEntry 列表到前端
                    │
                    └─> 渲染列表
                            │
                            └─> 用户点击某条记录
                                    │
                                    └─> Tauri 命令: copy_to_clipboard(content)
                                            │
                                            └─> ClipboardManager.write(content)
```

## 技术选型

### 为什么选择 Tauri？

**优势：**
- 轻量级：相比 Electron，安装包体积小 10 倍以上
- 安全性：Rust 后端内存安全，无 Node.js 运行时风险
- 跨平台：一套代码支持 Windows/macOS/Linux
- 性能：原生系统 WebView，资源占用低

**权衡：**
- 生态不如 Electron 成熟
- 前端调试体验略逊于 Electron DevTools

### 为什么选择 mDNS？

**优势：**
- 标准协议，跨平台支持
- 零配置，无需中心服务器
- 局域网自动发现

**权衡：**
- 依赖系统服务（Windows Bonjour, Linux Avahi）
- 某些网络环境可能禁用组播

**替代方案：**
- 手动输入 IP（v0.2 计划作为降级方案）

### 为什么选择 WebSocket？

**优势：**
- 全双工实时通信
- 支持心跳机制，及时检测断线
- 跨平台支持良好

**权衡：**
- 相比 UDP 有额外开销（TCP 握手）
- 不支持组播（需要点对点连接）

### 为什么选择 SQLite？

**优势：**
- 嵌入式，无需独立进程
- 成熟稳定，支持事务
- 索引支持，查询性能优秀

**权衡：**
- 并发写入能力有限（单写）
- 数据库文件需要手动管理

### 为什么选择固定轮询？

**优势：**
- 实现简单，跨平台一致
- CPU 占用可控（< 1%）
- 延迟可接受（500ms）

**权衡：**
- 理论最大延迟 500ms
- 相比事件驱动略耗资源

**替代方案：**
- 系统粘贴板事件（平台 API 不一致，macOS 需要私有 API）

## 目录结构

```
PaseBoard/
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── main.rs             # 程序入口
│   │   ├── app.rs              # 应用协调器
│   │   ├── network/            # 网络模块
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs      # 网络管理器
│   │   │   ├── mdns.rs         # mDNS 服务
│   │   │   ├── websocket.rs    # WebSocket 连接
│   │   │   └── protocol.rs     # 消息协议
│   │   ├── clipboard/          # 粘贴板模块
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs      # 粘贴板管理器
│   │   │   └── monitor.rs      # 粘贴板监听器
│   │   ├── storage/            # 存储模块
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs      # 存储管理器
│   │   │   └── models.rs       # 数据模型
│   │   └── config/             # 配置模块
│   │       ├── mod.rs
│   │       └── manager.rs      # 配置管理器
│   ├── Cargo.toml              # Rust 依赖
│   ├── tauri.conf.json         # Tauri 配置
│   └── icons/                  # 应用图标
├── ui/                         # 前端界面
│   ├── index.html              # 主界面
│   ├── styles.css              # 样式
│   └── app.js                  # 前端逻辑
├── README.md                   # 项目说明
├── ARCHITECTURE.md             # 本文档
├── CHANGELOG.md                # 更新日志
└── CLAUDE.md                   # AI 辅助开发指南
```

## 关键决策记录

详细的设计决策请参阅 `openspec/changes/paseboard-initial-implementation/design.md`。

### Decision 1: 全连接 P2P 网络

每台设备与所有其他设备建立双向连接。

- **优势**：无单点故障、低延迟
- **劣势**：连接数 O(N²)，建议设备数 ≤ 10

### Decision 2: 双重去重机制

UUID 去重 + 内容哈希去重 + 来源标记。

- **优势**：可靠防止消息回环
- **劣势**：略增加计算开销

### Decision 3: 混合错误处理

核心模块使用 `thiserror`，应用层使用 `anyhow`。

- **优势**：核心模块类型安全，应用层简洁
- **劣势**：需维护两套风格

## 性能指标

**目标：**
- 内存占用：< 50MB
- 空闲 CPU：< 1%
- 同步延迟：< 1 秒（1KB 内容）
- 启动时间：< 2 秒

**实测：**
（待补充实际测试数据）

## 安全考虑

**当前状态（v0.1）：**
- 数据明文传输（局域网环境）
- 无身份验证（信任局域网内所有设备）
- 无访问控制（所有连接设备可读写）

**计划改进（v0.2+）：**
- 端到端加密（AES-256-GCM）
- 设备配对机制（扫描二维码或输入配对码）
- 内容过滤（敏感信息检测）

## 扩展性

**已知限制：**
- 设备数：建议 ≤ 10 台
- 内容大小：≤ 10MB
- 内容类型：纯文本

**未来扩展方向：**
- 支持图片、文件
- 混合拓扑（星型中继）
- 移动端客户端
- 云端备份（可选）

## 故障处理

**连接断开：**
- 自动重连机制（指数退避：1s, 2s, 4s, 8s, 最多 30s）
- 重连期间消息缓存（队列最大 100 条）

**mDNS 服务不可用：**
- 启动时检测，不可用时显示错误提示
- 降级方案：手动输入 IP（v0.2 计划）

**数据库损坏：**
- 启动时检查数据库完整性
- 损坏时备份旧数据库，创建新数据库

**粘贴板访问失败：**
- 记录错误日志，不中断应用运行
- 重试机制（最多 3 次）

## 测试策略

**单元测试：**
- 核心模块功能测试（网络、粘贴板、存储）
- 去重逻辑测试
- 错误处理测试

**集成测试：**
- 多设备连接测试
- 消息同步端到端测试
- 断线重连测试

**性能测试：**
- 内存占用测试（长时间运行）
- CPU 占用测试（空闲 + 高频同步）
- 大内容传输测试（接近 10MB）

## 贡献指南

**代码规范：**
- Rust: `cargo fmt` + `cargo clippy`
- 注释：中文
- Commit message: 中文

**分支策略：**
- `main`: 稳定版本
- `develop`: 开发分支
- `feature/*`: 功能分支
- `fix/*`: 修复分支

**PR 流程：**
1. Fork 仓库
2. 创建功能分支
3. 提交代码（包含测试）
4. 通过 CI 检查
5. 提交 PR

## 参考资料

- [Tauri 文档](https://tauri.app/)
- [mDNS RFC 6762](https://datatracker.ietf.org/doc/html/rfc6762)
- [WebSocket RFC 6455](https://datatracker.ietf.org/doc/html/rfc6455)
- [SQLite 文档](https://www.sqlite.org/docs.html)

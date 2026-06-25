## Why

PaseBoard 解决多设备工作场景下的粘贴板同步痛点。开发者、写作者、设计师经常在多台设备间切换，需要频繁复制粘贴内容，但现有方案要么依赖云服务（隐私风险），要么需要复杂配置。PaseBoard 提供零配置、局域网内的实时粘贴板同步，无需账号、无需服务器，启动即用。

## What Changes

- **新增设备发现能力**：基于 mDNS 的局域网设备自动发现，无需手动配置
- **新增实时同步能力**：WebSocket 实现设备间粘贴板内容实时推送（< 1秒延迟）
- **新增粘贴板监听**：500ms 轮询监听系统粘贴板变化，自动推送到其他设备
- **新增历史记录管理**：SQLite 存储最近 1000 条粘贴板历史，支持查询和回溯
- **新增桌面 UI**：Tauri 桌面应用，系统托盘常驻，历史记录查看界面
- **新增消息去重机制**：双重保险（UUID + 内容哈希）防止消息回环
- **新增内容大小限制**：单次同步最大 10MB，超过则显示警告

## Capabilities

### New Capabilities

- `device-discovery`: mDNS 设备发现，局域网内设备自动发现与连接管理
- `realtime-sync`: WebSocket 实时通信，设备间粘贴板内容双向同步
- `clipboard-monitoring`: 粘贴板监听与写入，检测内容变化并触发同步
- `history-storage`: 历史记录存储与查询，SQLite 数据库管理
- `desktop-ui`: Tauri 桌面界面，系统托盘、设备列表、历史记录视图
- `message-deduplication`: 消息去重，防止回环和重复推送

### Modified Capabilities

<!-- 无现有能力需要修改，这是全新项目 -->

## Impact

**新增依赖：**
- Rust 后端：`tokio`, `tokio-tungstenite`, `mdns-sd`, `rusqlite`, `serde`, `serde_json`, `tauri`, `tauri-plugin-clipboard`
- 前端：原生 HTML/CSS/JavaScript（无框架依赖）

**新增文件结构：**
- `src/` - Rust 后端代码（网络层、粘贴板层、存储层）
- `tests/` - 单元测试、集成测试、E2E 测试
- `src-tauri/` - Tauri 配置和主进程代码
- `ui/` - 前端界面代码

**性能要求：**
- 设备发现延迟 < 3秒
- 消息传输延迟 < 1秒
- 历史查询响应 < 100ms
- 内存占用 < 50MB

**安全考虑：**
- MVP 阶段无端到端加密（局域网明文传输）
- 日志中不记录粘贴板内容原文（隐私保护）
- 不上传任何数据到云端

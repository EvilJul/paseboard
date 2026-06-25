## 1. 项目初始化

- [ ] 1.1 创建 Tauri 项目结构（`cargo tauri init`）
- [ ] 1.2 配置 Cargo.toml 依赖（tokio, tokio-tungstenite, mdns-sd, rusqlite, serde, serde_json, tauri-plugin-clipboard）
- [ ] 1.3 创建模块目录结构（src/network/, src/clipboard/, src/utils/）
- [ ] 1.4 创建配置管理模块（src/config.rs）

## 2. 错误处理与日志

- [ ] 2.1 定义核心错误类型（utils/error.rs: NetworkError, ClipboardError, StorageError）
- [ ] 2.2 配置日志框架（log + env_logger）
- [ ] 2.3 实现错误上下文传播（使用 thiserror 和 anyhow）

## 3. 设备发现（device-discovery）

- [ ] 3.1 实现 mDNS 服务注册（network/mdns.rs: 服务类型 _paseboard._tcp.local）
- [ ] 3.2 实现端口冲突自动降级（9527 → 9528 → ... → 9537）
- [ ] 3.3 实现设备信息广播（设备 ID、设备名称、实际端口）
- [ ] 3.4 实现 mDNS 设备监听与解析
- [ ] 3.5 实现设备列表管理（新增、更新、移除设备）
- [ ] 3.6 添加设备发现单元测试

## 4. WebSocket 通信（realtime-sync）

- [ ] 4.1 实现 WebSocket 服务端（network/websocket_server.rs）
- [ ] 4.2 实现 WebSocket 客户端（network/websocket_client.rs）
- [ ] 4.3 定义消息协议（network/message.rs: Message 结构体，JSON 序列化）
- [ ] 4.4 实现消息编解码共享逻辑（network/websocket_common.rs）
- [ ] 4.5 实现心跳检测机制（30 秒间隔，60 秒超时）
- [ ] 4.6 实现断线重连机制（指数退避，最多 3 次）
- [ ] 4.7 实现内容大小限制检查（10MB 上限）
- [ ] 4.8 实现消息广播优化（序列化一次 + 并发发送）
- [ ] 4.9 添加 WebSocket 单元测试和集成测试

## 5. 粘贴板监听与写入（clipboard-monitoring）

- [ ] 5.1 实现粘贴板监听器（clipboard/monitor.rs: 500ms 轮询）
- [ ] 5.2 实现内容哈希计算与去重（SHA256）
- [ ] 5.3 实现消息来源标记机制（区分本地 vs 网络）
- [ ] 5.4 实现粘贴板写入（clipboard/writer.rs）
- [ ] 5.5 实现写入失败重试逻辑（最多 3 次）
- [ ] 5.6 实现 UUID 消息去重缓存（最多 1000 条）
- [ ] 5.7 添加粘贴板层单元测试

## 6. 历史记录存储（history-storage）

- [ ] 6.1 定义 SQLite Schema（clipboard_history 表）
- [ ] 6.2 实现数据库初始化（clipboard/storage.rs）
- [ ] 6.3 实现索引创建（idx_timestamp, idx_content_hash）
- [ ] 6.4 实现历史记录插入
- [ ] 6.5 实现历史容量管理（1000 条上限，FIFO 删除）
- [ ] 6.6 实现历史查询（按时间倒序，限制条数）
- [ ] 6.7 添加存储层单元测试

## 7. 消息去重机制（message-deduplication）

- [ ] 7.1 集成 UUID 去重到消息接收流程
- [ ] 7.2 集成内容哈希去重到监听流程
- [ ] 7.3 实现双重保险验证逻辑
- [ ] 7.4 实现去重缓存大小限制
- [ ] 7.5 添加去重机制集成测试

## 8. 应用主逻辑协调

- [ ] 8.1 实现应用入口（src/main.rs）
- [ ] 8.2 实现模块协调逻辑（src/app.rs: 初始化各模块，连接事件）
- [ ] 8.3 实现并行初始化优化（mDNS、WebSocket、Storage 并行启动）
- [ ] 8.4 实现设备发现到连接建立的完整流程
- [ ] 8.5 实现粘贴板监听到消息推送的完整流程
- [ ] 8.6 实现消息接收到粘贴板写入的完整流程

## 9. 桌面 UI（desktop-ui）

- [ ] 9.1 创建 Tauri 主窗口配置
- [ ] 9.2 实现系统托盘图标和菜单
- [ ] 9.3 实现设备列表界面（HTML/CSS/JS）
- [ ] 9.4 实现历史记录界面（显示最近 100 条）
- [ ] 9.5 实现点击历史记录复制功能
- [ ] 9.6 实现内容预览截断显示（100 字符 + "..."）
- [ ] 9.7 实现相对时间显示（"2 分钟前"）
- [ ] 9.8 实现窗口关闭最小化到托盘
- [ ] 9.9 实现警告提示 UI（内容超过 10MB）
- [ ] 9.10 实现 Tauri IPC 命令（查询设备列表、查询历史记录）

## 10. 集成测试

- [ ] 10.1 编写 mDNS 发现 + WebSocket 连接集成测试
- [ ] 10.2 编写监听 + 消息生成 + WebSocket 发送集成测试
- [ ] 10.3 编写 WebSocket 接收 + 写入粘贴板集成测试
- [ ] 10.4 编写双重去重机制集成测试
- [ ] 10.5 编写历史存储 + 容量管理集成测试

## 11. E2E 测试

- [ ] 11.1 编写设备发现与连接 E2E 测试（2 台设备）
- [ ] 11.2 编写粘贴板同步 E2E 测试（设备 A → 设备 B）
- [ ] 11.3 编写快速连续复制 E2E 测试（100ms 间隔 3 次）
- [ ] 11.4 编写历史记录查看 E2E 测试

## 12. 性能优化与验证

- [ ] 12.1 添加性能基准测试（消息序列化、内容哈希、历史查询）
- [ ] 12.2 验证设备发现延迟 < 3 秒
- [ ] 12.3 验证消息传输延迟 < 1 秒
- [ ] 12.4 验证历史查询响应 < 100ms
- [ ] 12.5 验证内存占用 < 50MB
- [ ] 12.6 验证 CPU 占用（空闲）< 1%

## 13. 文档与打包

- [ ] 13.1 编写 README.md（功能说明、依赖要求、安装步骤）
- [ ] 13.2 编写 ARCHITECTURE.md（架构说明、模块职责）
- [ ] 13.3 配置 Tauri 打包（Windows .msi, macOS .dmg, Linux .deb/.rpm）
- [ ] 13.4 测试安装包在各平台的安装与运行
- [ ] 13.5 编写 CHANGELOG.md（v0.1 版本说明）

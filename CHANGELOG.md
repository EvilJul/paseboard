# Changelog

本文档记录 PaseBoard 的所有重要变更。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.1] - 2026-06-27

### 修复

- **修复 ACL 权限拒绝错误**：点击历史记录的"复制"按钮时不再报错 `plugin:clipboard-manager|write_text not allowed by ACL`。新增 Tauri v2 capability 配置，授权前端调用 clipboard 写入能力。窗口对象现在显式声明 `label: "main"`，与 capability 中的 `windows` 字段显式匹配，消除了隐式契约。

## [0.1.0] - 2026-06-26

### 首个版本

PaseBoard v0.1.0 是首个公开发布的版本，实现了核心的局域网粘贴板同步功能。

### 新增功能

#### 核心功能
- **零配置设备发现**：通过 mDNS 协议自动发现局域网内的其他设备
- **实时粘贴板同步**：粘贴板内容在 1 秒内同步到所有连接的设备
- **历史记录管理**：保存最近 1000 条粘贴板历史，支持查询和恢复
- **设备管理**：查看当前连接的设备列表，显示设备名称和连接状态

#### 用户界面
- **系统托盘集成**：最小化到系统托盘，快速访问设备列表
- **主窗口界面**：
  - 设备列表视图（显示设备名称、IP 地址、连接状态）
  - 历史记录视图（按时间倒序显示，支持点击恢复）
  - 设置面板（配置设备名称、自动启动、通知选项）

#### 技术特性
- **跨平台支持**：Windows 10+、macOS 10.15+、Linux（主流发行版）
- **轻量高效**：内存占用 < 50MB，空闲 CPU 占用 < 1%
- **可靠传输**：WebSocket 长连接 + 心跳机制，自动重连
- **去重保护**：三重去重机制（UUID、内容哈希、来源标记）防止消息回环

### 平台支持

- **Windows**：`.msi` 安装包（x64）
- **macOS**：`.dmg` 安装包（x64，支持 Intel 和 Apple Silicon）
- **Linux**：
  - Debian/Ubuntu: `.deb` 包
  - Fedora/RHEL: `.rpm` 包
  - 通用: `.AppImage`

### 依赖要求

- **Windows**：无需额外依赖（可选安装 Bonjour 优化设备发现）
- **macOS**：无需额外依赖（系统内置 mDNS 支持）
- **Linux**：需安装 Avahi（`sudo apt-get install avahi-daemon`）

### 已知限制

此版本存在以下限制，将在未来版本中改进：

- **内容类型限制**：仅支持纯文本，不支持图片、富文本、文件
- **安全性限制**：数据在局域网内明文传输，无加密保护
- **内容大小限制**：单次同步最大支持 10MB 文本内容
- **设备数量限制**：建议同时连接不超过 10 台设备（全连接网络性能限制）
- **网络限制**：仅支持同一局域网内设备，不支持跨网段或公网同步

### 技术实现

- **框架**：Tauri v1
- **后端语言**：Rust 1.70+
- **异步运行时**：Tokio
- **设备发现**：mDNS (mdns-sd 0.7.x)
- **实时通信**：WebSocket (tokio-tungstenite)
- **数据存储**：SQLite 3 (rusqlite)
- **粘贴板**：arboard

### 性能指标

- **启动时间**：< 2 秒
- **内存占用**：空闲状态 30-40MB
- **CPU 占用**：空闲状态 < 1%
- **同步延迟**：1KB 内容 < 500ms，10MB 内容 < 2 秒（局域网千兆网络）

### 安装说明

详细的安装和使用说明请参阅 [README.md](./README.md)。

### 架构文档

详细的架构设计和技术决策请参阅 [ARCHITECTURE.md](./ARCHITECTURE.md)。

---

## 未来计划

以下功能计划在未来版本中实现：

### [0.2.0] - 计划中

**安全增强：**
- 端到端加密（AES-256-GCM）
- 设备配对机制（扫描二维码或输入配对码）
- 可选的访问控制（白名单/黑名单）

**功能扩展：**
- 支持图片同步（PNG, JPEG, GIF）
- 支持富文本格式保留（HTML 粘贴板）
- 手动 IP 输入（mDNS 不可用时的降级方案）
- 历史记录全文搜索

**用户体验：**
- 通知系统（内容同步提示）
- 快捷键支持（快速打开历史记录）
- 深色模式
- 多语言支持（英文、中文）

### [0.3.0] - 长期计划

- 文件传输支持（< 100MB）
- 混合拓扑网络（支持更多设备）
- 移动端客户端（iOS, Android）
- 可选的云端备份

---

## 问题反馈

如果您在使用过程中遇到问题，请通过以下方式反馈：

- **Bug 报告**：[GitHub Issues](https://github.com/yourusername/paseboard/issues)
- **功能建议**：[GitHub Discussions](https://github.com/yourusername/paseboard/discussions)

---

## 贡献者

感谢所有为 PaseBoard 做出贡献的开发者！

---

[0.1.0]: https://github.com/yourusername/paseboard/releases/tag/v0.1.0

# PaseBoard

一个零配置的局域网粘贴板同步工具，让多设备间的内容传输变得简单高效。

## 核心特性

- **零配置启动**：无需手动设置，启动即可自动发现局域网内的其他设备
- **实时同步**：粘贴板内容在 1 秒内同步到所有连接的设备
- **历史记录**：保存最近 1000 条粘贴板历史，随时查询和恢复
- **轻量高效**：内存占用 < 50MB，空闲 CPU 占用 < 1%
- **跨平台支持**：统一代码库支持 Windows、macOS、Linux

## 解决的问题

在多设备工作场景下，你是否遇到过这些烦恼？
- 在笔记本上复制一段代码，想在台式机上粘贴，只能通过聊天软件传输
- 需要在多台设备间频繁传输文本内容，效率低下
- 现有的云粘贴板工具依赖公网，速度慢且有安全顾虑

PaseBoard 通过局域网直连解决这些问题，无需配置、无需登录、无需公网。

## 系统要求

### Windows
- **系统版本**：Windows 10 或更高版本
- **依赖**：无需额外依赖（可选安装 Bonjour 以优化设备发现）

### macOS
- **系统版本**：macOS 10.15 (Catalina) 或更高版本
- **依赖**：无需额外依赖（系统内置 mDNS 支持）

### Linux
- **系统版本**：主流发行版（Ubuntu 20.04+, Fedora 34+, Debian 11+ 等）
- **依赖**：需安装 Avahi 以支持 mDNS 设备发现
  ```bash
  # Ubuntu/Debian
  sudo apt-get install avahi-daemon
  
  # Fedora/RHEL
  sudo dnf install avahi
  
  # Arch Linux
  sudo pacman -S avahi
  ```

## 安装

### 下载安装包

访问 [Releases](https://github.com/fushengorg/paseboard/releases) 页面下载对应平台的安装包：

- **Windows**: `PaseBoard_0.1.0_x64-setup.exe`
- **macOS**: `PaseBoard_0.1.0_universal.dmg`（支持 Intel + Apple Silicon）
- **Linux**:
  - Debian/Ubuntu: `paseboard_0.1.0_amd64.deb`
  - Fedora/RHEL: `paseboard-0.1.0-1.x86_64.rpm`
  - 通用: `paseboard_0.1.0_amd64.AppImage`

### 安装步骤

**Windows:**
双击 `.msi` 文件，按照安装向导完成安装。

**macOS:**
1. 双击 `.dmg` 文件打开
2. 将 PaseBoard 拖拽到 Applications 文件夹
3. 首次运行时可能需要在"系统偏好设置 → 安全性与隐私"中允许

**Linux (Debian/Ubuntu):**
```bash
sudo dpkg -i paseboard_0.1.0_amd64.deb
```

**Linux (Fedora/RHEL):**
```bash
sudo rpm -i paseboard-0.1.0-1.x86_64.rpm
```

**Linux (AppImage):**
```bash
chmod +x paseboard_0.1.0_amd64.AppImage
./paseboard_0.1.0_amd64.AppImage
```

## 使用说明

### 启动应用

安装完成后，启动 PaseBoard 应用。应用会：
1. 自动在系统托盘显示图标
2. 自动发现局域网内其他运行 PaseBoard 的设备
3. 自动建立连接并开始同步

### 基本操作

**查看设备列表：**
点击系统托盘图标，查看当前连接的设备。

**复制同步：**
在任意设备上复制文本内容，内容会自动同步到所有连接的设备。

**查看历史记录：**
双击托盘图标打开主窗口，在历史记录列表中查看和恢复之前的内容。

**设置：**
在主窗口中可以配置：
- 设备名称
- 自动启动
- 通知设置

## 已知限制

- **内容类型**：当前版本仅支持纯文本，不支持图片、文件等
- **安全性**：数据在局域网内明文传输，请勿在不受信任的网络环境使用
- **内容大小**：单次同步最大支持 10MB 文本
- **设备数量**：建议同时连接不超过 10 台设备以保证性能

## 开发构建

### 前置要求

- Rust 1.70 或更高版本
- Node.js 16+ (如果需要修改 UI)
- Cargo Tauri CLI

### 克隆仓库

```bash
git clone https://github.com/yourusername/paseboard.git
cd paseboard
```

### 开发模式

```bash
# 运行开发服务器
cargo tauri dev
```

### 生产构建

```bash
# 构建生产版本
cargo tauri build
```

构建完成后，安装包位于 `src-tauri/target/release/bundle/` 目录。

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --package paseboard-backend --lib network::tests
```

## 技术栈

- **框架**: Tauri v1
- **后端**: Rust + Tokio
- **设备发现**: mDNS (mdns-sd)
- **实时通信**: WebSocket (tokio-tungstenite)
- **数据存储**: SQLite (rusqlite)
- **粘贴板**: arboard

## 架构文档

详细的架构说明请参阅 [ARCHITECTURE.md](./ARCHITECTURE.md)。

## 更新日志

详细的版本更新信息请参阅 [CHANGELOG.md](./CHANGELOG.md)。

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

## 联系方式

- 问题反馈：[GitHub Issues](https://github.com/yourusername/paseboard/issues)
- 功能建议：[GitHub Discussions](https://github.com/yourusername/paseboard/discussions)

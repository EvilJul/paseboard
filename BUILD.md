# 打包构建说明

本文档说明如何构建 PaseBoard 的各平台安装包。

## 前置要求

### 通用要求
- Rust 1.70 或更高版本
- Node.js 16+ (可选，如果需要修改 UI)
- Tauri CLI: `cargo install tauri-cli`

### 平台特定要求

**Windows:**
- Visual Studio 2019 或更高版本（包含 C++ 工具）
- WiX Toolset v3.11+（用于生成 .msi 安装包）

**macOS:**
- Xcode 命令行工具：`xcode-select --install`

**Linux:**
- 基础构建工具：
  ```bash
  # Debian/Ubuntu
  sudo apt-get install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
  
  # Fedora
  sudo dnf install webkit2gtk3-devel.x86_64 \
    openssl-devel \
    curl \
    wget \
    libappindicator-gtk3 \
    librsvg2-devel
  
  # Arch Linux
  sudo pacman -S webkit2gtk \
    base-devel \
    curl \
    wget \
    openssl \
    appmenu-gtk-module \
    gtk3 \
    libappindicator-gtk3 \
    librsvg
  ```

## 构建命令

### 开发构建（快速测试）

```bash
# 开发模式运行（带热重载）
cargo tauri dev

# 构建调试版本（不打包）
cargo build
```

### 生产构建（创建安装包）

```bash
# 构建所有平台支持的安装包格式
cargo tauri build

# 构建特定格式
cargo tauri build --target msi      # Windows MSI
cargo tauri build --target dmg      # macOS DMG
cargo tauri build --target deb      # Linux Debian
cargo tauri build --target rpm      # Linux RPM
cargo tauri build --target appimage # Linux AppImage
```

### 构建输出位置

构建完成后，安装包位于：

```
src-tauri/target/release/bundle/
├── msi/                        # Windows
│   └── PaseBoard_0.1.0_x64_en-US.msi
├── dmg/                        # macOS
│   └── PaseBoard_0.1.0_x64.dmg
├── deb/                        # Linux Debian
│   └── paseboard_0.1.0_amd64.deb
├── rpm/                        # Linux RPM
│   └── paseboard-0.1.0-1.x86_64.rpm
└── appimage/                   # Linux AppImage
    └── paseboard_0.1.0_amd64.AppImage
```

## 验证安装包

### Windows (.msi)

```powershell
# 安装
msiexec /i PaseBoard_0.1.0_x64_en-US.msi

# 静默安装
msiexec /i PaseBoard_0.1.0_x64_en-US.msi /quiet

# 卸载
msiexec /x PaseBoard_0.1.0_x64_en-US.msi
```

**验证项：**
- [ ] 安装到 `C:\Program Files\PaseBoard\`
- [ ] 开始菜单快捷方式创建成功
- [ ] 应用可正常启动
- [ ] 系统托盘图标显示正常
- [ ] 卸载后文件清理干净（配置文件保留在 `%USERPROFILE%\.paseboard\`）

### macOS (.dmg)

```bash
# 挂载 DMG
hdiutil attach PaseBoard_0.1.0_x64.dmg

# 复制到 Applications
cp -R /Volumes/PaseBoard/PaseBoard.app /Applications/

# 卸载 DMG
hdiutil detach /Volumes/PaseBoard
```

**验证项：**
- [ ] DMG 文件可正常打开
- [ ] 拖拽安装到 Applications 成功
- [ ] 首次启动通过 Gatekeeper 验证（可能需要在"安全性与隐私"中允许）
- [ ] 系统托盘图标显示正常
- [ ] 删除 .app 后应用卸载完成（配置文件保留在 `~/.paseboard/`）

### Linux (.deb)

```bash
# 安装
sudo dpkg -i paseboard_0.1.0_amd64.deb

# 修复依赖（如果有缺失）
sudo apt-get install -f

# 启动
paseboard

# 卸载
sudo dpkg -r paseboard
```

**验证项：**
- [ ] 依赖检查（avahi-daemon 应自动安装）
- [ ] 安装到 `/usr/bin/paseboard`
- [ ] 应用菜单快捷方式创建成功
- [ ] 应用可正常启动
- [ ] 系统托盘图标显示正常
- [ ] 卸载后文件清理干净（配置文件保留在 `~/.paseboard/`）

### Linux (.rpm)

```bash
# 安装
sudo rpm -i paseboard-0.1.0-1.x86_64.rpm

# 或使用 dnf
sudo dnf install paseboard-0.1.0-1.x86_64.rpm

# 启动
paseboard

# 卸载
sudo rpm -e paseboard
```

**验证项：**
- [ ] 依赖检查（avahi 应自动安装）
- [ ] 安装到 `/usr/bin/paseboard`
- [ ] 应用菜单快捷方式创建成功
- [ ] 应用可正常启动
- [ ] 系统托盘图标显示正常
- [ ] 卸载后文件清理干净（配置文件保留在 `~/.paseboard/`）

### Linux (.AppImage)

```bash
# 添加执行权限
chmod +x paseboard_0.1.0_amd64.AppImage

# 运行
./paseboard_0.1.0_amd64.AppImage
```

**验证项：**
- [ ] 无需安装直接运行
- [ ] 应用可正常启动
- [ ] 系统托盘图标显示正常
- [ ] 配置文件创建在 `~/.paseboard/`

## 打包配置说明

打包配置位于 `src-tauri/tauri.conf.json` 的 `bundle` 部分：

```json
{
  "bundle": {
    "active": true,
    "targets": ["msi", "dmg", "deb", "rpm", "appimage"],
    "identifier": "com.paseboard.app",
    "icon": [...],
    "category": "Utility",
    "shortDescription": "零配置局域网粘贴板同步工具",
    "longDescription": "...",
    "deb": {
      "depends": ["avahi-daemon"]
    },
    "macOS": {
      "minimumSystemVersion": "10.15"
    }
  }
}
```

### 关键配置项

- **identifier**: 应用唯一标识符（反向域名格式）
- **targets**: 要生成的安装包类型
- **icon**: 应用图标文件路径（各平台格式）
- **category**: 应用分类（Linux 桌面环境使用）
- **deb.depends**: Debian 包依赖（Avahi 用于 mDNS）
- **macOS.minimumSystemVersion**: macOS 最低系统版本

## 代码签名（可选）

### macOS 签名

```bash
# 配置签名证书
# 在 tauri.conf.json 中设置 macOS.signingIdentity

cargo tauri build -- --target universal-apple-darwin
```

### Windows 签名

```bash
# 配置证书指纹
# 在 tauri.conf.json 中设置 windows.certificateThumbprint

cargo tauri build
```

**注意**: 代码签名需要有效的开发者证书，对于开源项目可以跳过此步骤。

## 持续集成（CI）

### GitHub Actions 示例

```yaml
name: Build

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install dependencies (Linux)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.0-dev \
            build-essential curl wget libssl-dev \
            libgtk-3-dev libayatana-appindicator3-dev \
            librsvg2-dev
      
      - name: Build
        run: cargo tauri build
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: PaseBoard-${{ matrix.os }}
          path: src-tauri/target/release/bundle/
```

## 常见问题

### Q: 构建时提示缺少依赖

**A**: 检查平台特定要求部分，确保安装了所有必需的系统依赖。

### Q: Windows 构建失败，提示找不到 WiX

**A**: 下载并安装 WiX Toolset：https://wixtoolset.org/releases/

### Q: macOS 首次运行时提示"来自身份不明开发者"

**A**: 右键点击应用，选择"打开"，或在"系统偏好设置 → 安全性与隐私"中允许。

### Q: Linux 启动时提示 mDNS 不可用

**A**: 确保安装并启动 Avahi：
```bash
sudo apt-get install avahi-daemon
sudo systemctl start avahi-daemon
```

### Q: 构建的安装包体积过大

**A**: 确保使用 `cargo tauri build`（release 模式），不要使用 `cargo build`（debug 模式）。

### Q: 如何减小安装包体积

**A**: 
- 使用 `strip` 工具移除调试符号：`strip target/release/paseboard`
- 在 `Cargo.toml` 中启用 LTO 和优化：
  ```toml
  [profile.release]
  lto = true
  opt-level = "z"
  ```

## 发布检查清单

构建发布版本前，请确保：

- [ ] 更新 `CHANGELOG.md` 版本号和内容
- [ ] 更新 `src-tauri/tauri.conf.json` 中的 `version`
- [ ] 更新 `src-tauri/Cargo.toml` 中的 `version`
- [ ] 运行完整测试：`cargo test`
- [ ] 运行代码检查：`cargo clippy`
- [ ] 运行格式检查：`cargo fmt --check`
- [ ] 在所有目标平台上构建并测试安装包
- [ ] 验证安装包的安装、运行、卸载流程
- [ ] 创建 Git 标签：`git tag v0.1.0`
- [ ] 推送标签：`git push origin v0.1.0`
- [ ] 在 GitHub 创建 Release 并上传安装包

## 技术支持

如果在构建过程中遇到问题，请参考：

- [Tauri 官方文档](https://tauri.app/v1/guides/building/)
- [GitHub Issues](https://github.com/yourusername/paseboard/issues)

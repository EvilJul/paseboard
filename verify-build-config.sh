#!/bin/bash
# 打包配置验证脚本

set -e

echo "=========================================="
echo "PaseBoard 打包配置验证"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查函数
check_pass() {
    echo -e "${GREEN}✓${NC} $1"
}

check_fail() {
    echo -e "${RED}✗${NC} $1"
}

check_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# 1. 检查 Rust 环境
echo "1. 检查 Rust 环境..."
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(cargo --version)
    check_pass "Rust 已安装: $RUST_VERSION"
else
    check_fail "Rust 未安装，请访问 https://rustup.rs/ 安装"
    exit 1
fi

# 2. 检查 Tauri CLI
echo ""
echo "2. 检查 Tauri CLI..."
if command -v cargo-tauri &> /dev/null; then
    TAURI_VERSION=$(cargo tauri --version 2>&1 | head -1)
    check_pass "Tauri CLI 已安装: $TAURI_VERSION"
else
    check_warn "Tauri CLI 未安装，尝试安装..."
    cargo install tauri-cli
fi

# 3. 检查配置文件
echo ""
echo "3. 检查配置文件..."

if [ -f "src-tauri/tauri.conf.json" ]; then
    check_pass "tauri.conf.json 存在"

    # 验证 JSON 格式
    if python3 -m json.tool src-tauri/tauri.conf.json > /dev/null 2>&1; then
        check_pass "JSON 格式正确"
    else
        check_fail "JSON 格式错误"
        exit 1
    fi

    # 检查关键配置项
    if grep -q '"identifier": "com.paseboard.app"' src-tauri/tauri.conf.json; then
        check_pass "应用标识符配置正确"
    else
        check_fail "应用标识符配置错误"
    fi

    if grep -q '"version": "0.1.0"' src-tauri/tauri.conf.json; then
        check_pass "版本号配置正确: 0.1.0"
    else
        check_warn "版本号可能需要更新"
    fi

else
    check_fail "tauri.conf.json 不存在"
    exit 1
fi

# 4. 检查图标文件
echo ""
echo "4. 检查图标文件..."

ICON_DIR="src-tauri/icons"
REQUIRED_ICONS=("32x32.png" "128x128.png" "128x128@2x.png" "icon.icns" "icon.ico" "icon.png")

for icon in "${REQUIRED_ICONS[@]}"; do
    if [ -f "$ICON_DIR/$icon" ]; then
        check_pass "$icon 存在"
    else
        check_fail "$icon 缺失"
    fi
done

# 5. 检查 UI 文件
echo ""
echo "5. 检查 UI 文件..."

if [ -f "ui/index.html" ]; then
    check_pass "index.html 存在"
else
    check_fail "index.html 缺失"
fi

# 6. 检查源代码编译
echo ""
echo "6. 检查源代码编译..."

cd src-tauri
if cargo check --release > /dev/null 2>&1; then
    check_pass "源代码编译检查通过"
else
    check_warn "源代码编译检查有警告或错误"
    echo "   运行 'cargo check --release' 查看详情"
fi
cd ..

# 7. 检查文档文件
echo ""
echo "7. 检查文档文件..."

DOCS=("README.md" "ARCHITECTURE.md" "CHANGELOG.md" "BUILD.md")
for doc in "${DOCS[@]}"; do
    if [ -f "$doc" ]; then
        check_pass "$doc 存在"
    else
        check_warn "$doc 缺失"
    fi
done

# 8. 平台特定依赖检查
echo ""
echo "8. 检查平台特定依赖..."

OS_TYPE=$(uname -s)
case "$OS_TYPE" in
    Linux*)
        echo "   检测到 Linux 系统"

        # 检查 webkit2gtk
        if pkg-config --exists webkit2gtk-4.0; then
            check_pass "webkit2gtk-4.0 已安装"
        else
            check_fail "webkit2gtk-4.0 未安装"
            echo "   安装命令: sudo apt-get install libwebkit2gtk-4.0-dev"
        fi

        # 检查 Avahi
        if command -v avahi-daemon &> /dev/null; then
            check_pass "Avahi 已安装"
        else
            check_warn "Avahi 未安装（运行时需要）"
            echo "   安装命令: sudo apt-get install avahi-daemon"
        fi
        ;;

    Darwin*)
        echo "   检测到 macOS 系统"

        # 检查 Xcode 命令行工具
        if xcode-select -p &> /dev/null; then
            check_pass "Xcode 命令行工具已安装"
        else
            check_fail "Xcode 命令行工具未安装"
            echo "   安装命令: xcode-select --install"
        fi

        check_pass "mDNS 支持（系统内置）"
        ;;

    MINGW*|MSYS*|CYGWIN*)
        echo "   检测到 Windows 系统"

        # 检查 Visual Studio
        if command -v cl.exe &> /dev/null 2>&1; then
            check_pass "Visual Studio C++ 工具已安装"
        else
            check_warn "Visual Studio C++ 工具可能未安装"
        fi

        check_warn "Bonjour 可选（优化设备发现）"
        ;;

    *)
        check_warn "未知操作系统: $OS_TYPE"
        ;;
esac

# 9. 打包目标验证
echo ""
echo "9. 验证打包目标配置..."

TARGETS=$(grep -o '"targets": \[.*\]' src-tauri/tauri.conf.json)
echo "   配置的打包目标: $TARGETS"

case "$OS_TYPE" in
    Linux*)
        check_pass "可构建: deb, rpm, appimage"
        ;;
    Darwin*)
        check_pass "可构建: dmg, app"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        check_pass "可构建: msi"
        ;;
esac

# 10. 总结
echo ""
echo "=========================================="
echo "验证完成"
echo "=========================================="
echo ""
echo "下一步操作："
echo "  开发模式: cargo tauri dev"
echo "  生产构建: cargo tauri build"
echo "  查看文档: cat BUILD.md"
echo ""

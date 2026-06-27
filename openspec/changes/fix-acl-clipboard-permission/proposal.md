## Why

用户在两端安装 PaseBoard v0.1.0 后，点击历史记录的"复制"按钮时报错：

```
Command plugin:clipboard-manager|write_text not allowed by ACL
```

**根因**：`src-tauri/capabilities/` 目录不存在，`tauri.conf.json` 的 `app.security` 字段未声明任何 capability。Tauri v2 强制要求前端调用 Rust 插件命令前必须在 capabilities 中显式授权，否则一律拒绝——即使 Rust 侧已通过 `.plugin(tauri_plugin_clipboard_manager::init())` 注册了插件。

这是一个**纯配置缺失**，不是代码逻辑错误。修复后无需重启两端设备流程，复制按钮立即可用。

## What Changes

- **新建** `src-tauri/capabilities/default.json`：声明 `clipboard-manager:default`、`clipboard-manager:allow-write-text`、`clipboard-manager:allow-read-text` 三个权限，以及 `core:default` 等核心权限（确保窗口/事件/App 命令可用）
- **修改** `src-tauri/tauri.conf.json`：在 `app.security` 中添加 `"capabilities": ["default"]` 字段，关联新建的 capability 文件

无破坏性变更（不修改任何 Rust 代码、不修改 UI、不修改 WebSocket/mDNS 协议）。

## Capabilities

### New Capabilities

- `tauri-capabilities`: 定义 PaseBoard 前端可调用的 Tauri v2 插件命令权限白名单（capabilities 声明）。当前 scope 涵盖 clipboard-manager 读写权限以及核心窗口/事件/App 权限。

### Modified Capabilities

（无。现有 `openspec/specs/` 目录为空，无历史 spec 需要修改 REQUIREMENTS。）

## Impact

| 影响项 | 详情 |
|--------|------|
| **新增文件** | `src-tauri/capabilities/default.json`（1 个） |
| **修改文件** | `src-tauri/tauri.conf.json`（仅 `app.security.capabilities` 字段，1 处改动） |
| **Rust 代码** | 无修改 |
| **前端 UI** | 无修改 |
| **依赖** | 无新增依赖（`tauri-plugin-clipboard-manager = "2"` 已存在） |
| **数据库** | 无影响 |
| **网络协议** | 无影响 |
| **可观测性** | 修复后用户可在 UI 复制历史记录；无新日志/指标 |

**验证影响**：
- `cargo check` 通过 → 编译期验证 capability 配置被 Tauri 识别
- 手动跑 `cargo tauri dev` → 点击历史记录复制按钮 → 不再报 ACL 错误 → 粘贴板成功写入新内容

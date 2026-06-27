## Context

PaseBoard 是一个基于 Tauri v2 + Rust 的局域网粘贴板同步工具。前端（HTML/CSS/JS）通过 Tauri IPC 调用 Rust 命令及插件。Tauri v2 引入了**权限白名单机制（ACL）**：所有从前端到 Rust 的命令调用必须先在 capability 文件中显式声明，否则一律拒绝。

当前项目状态：
- `src-tauri/Cargo.toml` 已声明 `tauri-plugin-clipboard-manager = "2"`
- `src-tauri/src/main.rs` 已通过 `.plugin(tauri_plugin_clipboard_manager::init())` 注册插件（Rust 侧 OK）
- `src-tauri/capabilities/` 目录**不存在**
- `src-tauri/tauri.conf.json` 的 `app.security` 字段只有 `csp: null`，**没有 `capabilities` 字段**

这导致前端调用 `invoke('plugin:clipboard-manager|write_text', { text, label })` 时被 ACL 拒绝。

**修复范围**：仅修改配置文件，不动 Rust 代码、不动前端 UI。

## Goals / Non-Goals

**Goals：**
- 在 `src-tauri/capabilities/default.json` 中声明 clipboard-manager 读写权限
- 在 `tauri.conf.json` 中通过 `app.security.capabilities` 关联该 capability 文件
- 最小权限原则：只声明必需的权限，不开放额外敏感能力
- 编译期可验证（`cargo check` 报错即失败）
- 不破坏现有任何功能

**Non-Goals：**
- 不修改 Rust 源代码（`main.rs` / `app.rs` / 任何模块）
- 不修改前端 UI（`ui/index.html` / 任何 JS）
- 不修改 `Cargo.toml`（`tauri-plugin-clipboard-manager = "2"` 已正确）
- 不引入新的 Rust 依赖
- 不修改 WebSocket、mDNS、消息协议
- 不修改数据库 schema

## Decisions

### 决策 1：使用 `default.json` 作为 capability 文件名

**选择**：文件名固定为 `default.json`，`identifier` 字段为 `"default"`。

**理由**：
- Tauri v2 约定俗成的命名，团队成员可预测
- `tauri.conf.json` 的 `app.security.capabilities` 数组中直接引用字符串 `"default"`，语义清晰
- 未来如需分组（如 `clipboard.json` / `network.json`），可通过新增 capability 文件扩展

**替代方案**：
- 命名为 `clipboard.json` / `acl.json` 等更具体的名字 — 拒绝，因为当前只有一组权限，无分组必要

### 决策 2：权限粒度选择 `clipboard-manager:default` + 显式子权限

**选择**：
```json
"clipboard-manager:default",
"clipboard-manager:allow-write-text",
"clipboard-manager:allow-read-text"
```

**理由**：
- `clipboard-manager:default` 包含一组默认权限（随插件版本可能变化）
- 显式列出 `allow-write-text` 和 `allow-read-text` 是**双保险**：即使未来插件升级改变 `default` 的语义，写权限也不会丢失
- 匹配错误现象（`write_text` 被拒），明确授权写操作

**替代方案**：
- 只写 `clipboard-manager:default`（依赖插件默认）— 拒绝，依赖行为不够稳定
- 写 `clipboard-manager:allow-*` 全部 7 个权限 — 过度授权，违反最小权限原则

### 决策 3：核心权限使用 `core:default` 聚合

**选择**：`"core:default"` 一行覆盖窗口/事件/App 等核心命令。

**理由**：
- PaseBoard UI 依赖 Tauri 核心命令（窗口控制、事件监听等）
- `core:default` 是 Tauri v2 官方推荐的"开箱即用"组合
- 比手动列出 `core:window:default` / `core:event:default` 等 5+ 行更简洁

**替代方案**：
- 显式列出所有 `core:*` 子权限 — 拒绝，冗长且不必要

### 决策 4：Windows 列表限定为 `["main"]`

**选择**：`"windows": ["main"]`，仅授权主窗口。

**理由**：
- PaseBoard 当前只有一个 Tauri 窗口（在 `tauri.conf.json` 第 39-49 行定义）
- 限制为 `main` 是最小权限原则的标准实践
- 未来如需弹窗（如 PIN 输入对话框），可创建新窗口 + 新 capability

### 决策 5：`$schema` 字段使用相对路径

**选择**：
```json
"$schema": "../gen/schemas/desktop-schema.json"
```

**理由**：
- Tauri v2 构建时会在 `src-tauri/gen/schemas/` 生成 schema 文件
- 相对路径让 IDE（VS Code）能提供自动补全和类型检查
- 即使 schema 文件不存在也不影响构建（Tauri 不会因 schema 缺失报错）

## Risks / Trade-offs

| 风险 | 缓解措施 |
|------|---------|
| 权限标识符拼写错误导致 Tauri 启动时崩溃 | `cargo check` 阶段会暴露，验证步骤包含此检查 |
| 未来 `tauri-plugin-clipboard-manager` 升级改变 `default` 权限语义 | 显式列出 `allow-write-text` / `allow-read-text` 作为双保险 |
| 其他插件（如未来加 fs / shell）未授权导致功能不可用 | 当前 change 不涉及其他插件；后续添加新插件时按需扩展 capability |
| `$schema` 路径在某些环境下找不到 | Tauri 不强制要求 schema 存在，仅 IDE 辅助；构建不受影响 |
| 误开放过多权限（`clipboard-manager:allow-*` 全开） | 已限定为最小必要集合，code review 时关注 |

## Migration Plan

无数据迁移、无版本兼容性问题。修复步骤：

1. **创建** `src-tauri/capabilities/default.json`（一次性）
2. **修改** `src-tauri/tauri.conf.json` 的 `app.security` 字段（1 处）
3. **验证** `cargo check` 通过
4. **手动验证** 启动应用，点击历史记录复制按钮无 ACL 错误
5. **提交** git commit（中文明）

**回滚策略**：如果出现意外问题，回退 commit 即可。capability 文件独立，无外部依赖。

## Open Questions

（无。此修复根因明确，方案确定，无需进一步决策。）

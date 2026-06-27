## ADDED Requirements

### Requirement: 默认 Capability 文件必须存在

项目根的 `src-tauri/capabilities/default.json` 文件 MUST 存在，且 MUST 包含 `identifier: "default"` 字段。该文件声明了前端可调用的 Tauri v2 插件命令权限白名单。

#### Scenario: Capability 文件存在
- **WHEN** 构建 PaseBoard 时（`cargo tauri build` 或 `cargo tauri dev`）
- **THEN** Tauri MUST 能加载 `src-tauri/capabilities/default.json` 且不报"capability not found"错误

### Requirement: Clipboard-Manager 写入文本权限必须被授权

`default.json` 的 `permissions` 数组 MUST 包含 `clipboard-manager:allow-write-text` 权限标识符（或更宽松的 `clipboard-manager:default`）。

#### Scenario: 点击历史记录复制按钮
- **WHEN** 用户在 UI 上点击任一历史记录的"复制"按钮
- **THEN** 前端调用 `invoke('plugin:clipboard-manager|write_text', { text, ... })` MUST 成功
- **AND** 系统粘贴板 MUST 被写入对应文本内容
- **AND** 控制台 MUST 不出现 `not allowed by ACL` 错误

### Requirement: Clipboard-Manager 读取文本权限必须被授权

`default.json` 的 `permissions` 数组 MUST 包含 `clipboard-manager:allow-read-text` 权限标识符。

#### Scenario: 监听器读取粘贴板内容
- **WHEN** 后端 `ClipboardMonitor` 启动后调用读取粘贴板 API
- **THEN** 读取操作 MUST 成功，无 ACL 拒绝错误

### Requirement: 核心 Tauri 权限必须被授权

`default.json` 的 `permissions` 数组 MUST 包含 `core:default`（或其他等效的核心权限组合如 `core:window:default`、`core:event:default`、`core:app:default`）。

#### Scenario: 前端调用窗口和事件命令
- **WHEN** 前端通过 IPC 调用任意 Tauri 核心命令（如窗口操作、事件监听）
- **THEN** 调用 MUST 成功，无 ACL 拒绝

### Requirement: tauri.conf.json 必须引用 default capability

`src-tauri/tauri.conf.json` 的 `app.security.capabilities` 字段 MUST 是一个字符串数组，且 MUST 包含元素 `"default"`。

#### Scenario: 配置被加载
- **WHEN** Tauri 应用启动
- **THEN** MUST 从 `app.security.capabilities` 数组加载声明的 capability
- **AND** MUST 应用其权限白名单到所有前端调用

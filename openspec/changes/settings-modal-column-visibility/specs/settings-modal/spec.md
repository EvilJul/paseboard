## ADDED Requirements

### Requirement: Settings modal opens on gear button click
点击状态栏 ⚙️ 按钮时打开设置弹窗，替换原有下拉菜单行为

#### Scenario: User clicks gear button
- **WHEN** 用户点击状态栏的 ⚙️ 按钮
- **THEN** 打开居中 Modal 弹窗（z-index: 9000），显示半透明遮罩层

#### Scenario: User clicks overlay to close
- **WHEN** 用户点击弹窗外部遮罩层
- **THEN** 关闭弹窗

#### Scenario: User clicks close button
- **WHEN** 用户点击弹窗右上角 ✕ 按钮
- **THEN** 关闭弹窗

### Requirement: Settings modal displays grouped checkboxes
弹窗内按分组显示设置项

#### Scenario: Modal displays three groups
- **WHEN** 设置弹窗打开
- **THEN** 显示三个分组：设备列表（状态、IP+端口）、历史记录（来源设备、时间·大小）、其他（开发者模式）
- **AND** 每组有标题和分隔线
- **AND** "设备名称"和"内容预览"不在设置中显示（主键列，不可隐藏）

### Requirement: Settings modal visual style
弹窗使用深色主题

#### Scenario: Modal renders with dark theme
- **WHEN** 设置弹窗打开
- **THEN** 弹窗背景为 #1e1e2e，文字颜色为 #cdd6f4，圆角 12px，内边距 24px
- **AND** 版本号右对齐，12px，颜色 #6c7086

### Requirement: Developer mode toggle migrated to modal
开发者模式开关从下拉菜单迁入弹窗

#### Scenario: Toggle developer mode in modal
- **WHEN** 用户在弹窗中切换"开发者模式"checkbox
- **THEN** 控制台按钮显示/隐藏，状态持久化到 `paseboard:devMode`

#### Scenario: Shortcut works with modal open
- **WHEN** 用户按 Ctrl+Shift+D（弹窗打开或关闭状态）
- **THEN** 切换开发者模式，弹窗内的 checkbox 状态同步更新

## ADDED Requirements

### Requirement: Column visibility toggles via CSS data-col attributes
通过 CSS 属性选择器控制表格列的显隐

#### Scenario: Toggle column visibility
- **WHEN** 用户在设置弹窗中取消勾选某列的 checkbox
- **THEN** 对应表格中该列的所有 `<th>` 和 `<td>` 立即隐藏（`display: none`）
- **AND** 重新勾选后该列立即恢复显示

#### Scenario: All optional columns hidden
- **WHEN** 用户隐藏某表格的所有可选列
- **THEN** 表格仅剩主键列，布局不破损，主键列自动撑满表格宽度

### Requirement: Column visibility persists to localStorage
列显隐设置持久化到 localStorage

#### Scenario: Settings persist across restarts
- **WHEN** 用户隐藏某列后关闭应用并重启
- **THEN** 该列仍然隐藏

#### Scenario: First-time use defaults
- **WHEN** localStorage 中不存在 `paseboard:columnVisibility` key
- **THEN** 所有列默认可见

#### Scenario: Corrupted localStorage
- **WHEN** localStorage 中 `paseboard:columnVisibility` 的值不是有效 JSON
- **THEN** 降级为默认值（所有列可见），不崩溃

### Requirement: Column visibility applies to dynamic table content
列显隐对动态渲染的表格内容生效

#### Scenario: Column visibility maintained after auto-refresh
- **WHEN** 5 秒自动刷新重渲染表格后
- **THEN** 列显隐状态保持不变（`data-col` 属性在渲染时添加，CSS 规则持续生效）

#### Scenario: Both text and image history rows respect visibility
- **WHEN** 历史记录中同时存在文本和图片类型的行
- **THEN** 两种行模板的列显隐行为一致

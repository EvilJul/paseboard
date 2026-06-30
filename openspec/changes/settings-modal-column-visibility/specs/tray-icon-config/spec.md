## MODIFIED Requirements

### Requirement: Developer mode toggle location changed
开发者模式开关从下拉菜单迁移到设置弹窗

#### Scenario: Old dropdown removed
- **WHEN** 用户点击 ⚙️ 按钮
- **THEN** 不再显示下拉菜单，改为打开设置弹窗

#### Scenario: Developer mode toggle in new location
- **WHEN** 用户打开设置弹窗
- **THEN** "开发者模式" checkbox 显示在"其他"分组下
- **AND** 功能逻辑与原下拉菜单中的 toggle 完全一致

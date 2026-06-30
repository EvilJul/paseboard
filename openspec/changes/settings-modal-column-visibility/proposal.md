## Why

PaseBoard 的设备列表和历史记录表格各有 3 列，但并非所有列对每个用户都有用。单设备用户不需要"来源设备"列，只想看局域网 IP 的用户可能想隐藏"状态"列。当前没有地方控制这些列的显示/隐藏。此外，设置入口只有一个下拉菜单，只有一个"开发者模式"开关，扩展性差。

## What Changes

- 新增设置弹窗（Modal），替换现有 ⚙️ 下拉菜单
- 弹窗内新增列显隐 checkbox 控制：设备列表（状态、IP+端口）和历史记录（来源设备、时间·大小）
- "开发者模式"开关从下拉菜单迁入弹窗
- 列显隐通过 CSS `data-col` 属性选择器实现，持久化到 localStorage
- 删除现有 `.settings-dropdown` 下拉菜单 HTML 和 CSS

## Capabilities

### New Capabilities
- `settings-modal`: 设置弹窗 UI，包含分组 checkbox、遮罩层、打开/关闭交互
- `column-visibility`: 列显隐控制，通过 data-col 属性和 CSS class 切换实现，localStorage 持久化

### Modified Capabilities
- `tray-icon-config`: 开发者模式 toggle 从下拉菜单迁移到设置弹窗，功能逻辑不变

## Impact

- `ui/index.html`: HTML 结构（新增 Modal、删除下拉菜单）、CSS 样式（新增 Modal 样式、列显隐规则）、JS 逻辑（设置弹窗、列显隐、localStorage 读写、开发者模式迁移）
- 无后端改动，无新依赖，无 API 变更
- 配对弹窗 z-index 层级需要协调（设置弹窗 9000 < 配对弹窗 10000）

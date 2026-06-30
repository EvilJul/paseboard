## 1. 删除旧设置 UI

- [x] 1.1 删除 `.settings-dropdown` 下拉菜单 HTML 结构（`ui/index.html:669-676`）
- [x] 1.2 删除 `.settings-dropdown` 相关 CSS 样式（`ui/index.html:483-542`）
- [x] 1.3 删除 `initSettings()` 中的下拉菜单打开/关闭逻辑（`ui/index.html:999-1028`）

## 2. 新增设置弹窗 HTML 和 CSS

- [x] 2.1 添加设置弹窗 HTML 结构：`.settings-modal-overlay` + `.settings-modal-content`，包含三个分组（设备列表、历史记录、其他）、checkbox、版本号、✕ 关闭按钮
- [x] 2.2 添加设置弹窗 CSS 样式：深色主题（`#1e1e2e` 背景，`#cdd6f4` 文字），z-index: 9000，320px 宽度，居中定位
- [x] 2.3 添加列显隐 CSS 规则：`table.hide-devices-status [data-col="status"]` 等属性选择器

## 3. 表格添加 data-col 属性

- [x] 3.1 设备列表静态 `<thead>` 添加 `data-col` 属性（`data-col="status"`、`data-col="address"`）
- [x] 3.2 `loadDevices()` 渲染模板中 `<td>` 添加 `data-col` 属性
- [x] 3.3 历史记录静态 `<thead>` 添加 `data-col` 属性（`data-col="source"`、`data-col="time"`）
- [x] 3.4 `loadHistory()` 渲染模板中 `<td>` 添加 `data-col` 属性（文本行和图片行两种模板）

## 4. 设置弹窗 JS 逻辑

- [x] 4.1 实现 `loadColumnVisibility()`：从 localStorage 读取 `paseboard:columnVisibility`，try-catch 降级
- [x] 4.2 实现 `applyColumnVisibility(config)`：根据配置切换 table 的 CSS class
- [x] 4.3 实现 `openSettingsModal()` / `closeSettingsModal()`：打开/关闭弹窗，加载当前配置到 checkbox
- [x] 4.4 实现 checkbox change 事件：更新 localStorage + 调用 `applyColumnVisibility()`
- [x] 4.5 实现遮罩层点击关闭和 ✕ 按钮关闭

## 5. 开发者模式迁移

- [x] 5.1 将 `updateDevModeUI()` 逻辑迁移到设置弹窗的 checkbox 事件中
- [x] 5.2 确保 Ctrl+Shift+D 快捷键在弹窗打开/关闭状态下均有效，切换后同步更新弹窗内 checkbox

## 6. 测试验证

- [x] 6.1 验证列显隐即时生效（勾选/取消勾选后表格立即更新）
- [x] 6.2 验证持久化（重启后设置保持）
- [x] 6.3 验证边界情况（全部隐藏、localStorage 损坏、弹窗堆叠）
- [x] 6.4 运行 `cargo check` 确认编译通过

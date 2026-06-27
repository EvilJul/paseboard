## 1. 创建 capability 文件

- [x] 1.1 新建文件 `src-tauri/capabilities/default.json`，内容遵循 design.md 决策 1-5（identifier: default、windows: ["main"]、permissions 含 clipboard-manager 三项 + core:default）
- [x] 1.2 校验 JSON 语法合法（`python3 -m json.tool src-tauri/capabilities/default.json`）

## 2. 修改 tauri.conf.json

- [x] 2.1 在 `src-tauri/tauri.conf.json` 的 `app.security` 对象中添加 `"capabilities": ["default"]` 字段
- [x] 2.2 校验 JSON 语法合法（`python3 -m json.tool src-tauri/tauri.conf.json`）

## 3. 编译验证

- [x] 3.1 运行 `cd src-tauri && cargo check`，确认无 capability 相关编译错误
- [x] 3.2 如有 "permission not found" 类错误，调整 `default.json` 中权限标识符后重试

## 4. 手动验证

<!-- 任务 4.x 手动验证由用户执行 -->

- [ ] 4.1 运行 `cd src-tauri && cargo tauri dev` 启动开发模式
- [ ] 4.2 在 UI 上点击任一历史记录的"复制"按钮
- [ ] 4.3 确认：浏览器/系统粘贴板出现该历史记录内容，控制台无 `not allowed by ACL` 错误

## 5. 提交

- [x] 5.1 `git add src-tauri/capabilities/default.json src-tauri/tauri.conf.json`
- [x] 5.2 `git commit -m "fix: 修复 ACL 权限 - 添加 clipboard-manager capabilities 声明"`，commit message 详细说明根因和修复方式
- [x] 5.3 报告 commit hash 给主 agent

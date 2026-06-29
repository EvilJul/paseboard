## Context

macOS 上应用默认显示 Dock 图标（`NSApplicationActivationPolicyRegular`）。PaseBoard 以系统托盘为主交互入口，需要隐藏 Dock 图标（`NSApplicationActivationPolicyAccessory`）。

Tauri v2（2.11.3）原生暴露 `ActivationPolicy::Accessory` API，通过 `tauri::Builder` 或 `AppHandle::set_activation_policy()` 调用，无需 unsafe 代码。

## Goals / Non-Goals

**Goals:**
- macOS 上 PaseBoard 启动后 Dock 图标不显示
- 系统托盘图标正常交互

**Non-Goals:**
- 不改变 Windows/Linux 行为
- 不修改窗口显示/隐藏逻辑

## Decisions

**D1: 使用 `AppHandle::set_activation_policy()` 而非 `Builder::activation_policy()`**
   - 在 `setup` 闭包中调用，此时 app 已完全初始化
   - 备选：Builder 链式调用仅在 `run()` 前生效，无法在 setup 中重叠

**D2: `#[cfg(target_os = "macos")]` 内联条件编译**
   - 单行 cfg 属性，不引入独立函数
   - Windows/Linux 编译时完全跳过此行

## Risks / Trade-offs

- [低风险] 设置 `Accessory` 后应用无菜单栏 → PaseBoard 无需菜单栏，不影响
- [低风险] 窗口隐藏后用户只能通过托盘图标唤出 → 已是当前行为

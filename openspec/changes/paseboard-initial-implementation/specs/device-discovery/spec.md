## ADDED Requirements

### Requirement: 设备自动发现
系统 SHALL 在启动时自动发现局域网内的其他 PaseBoard 设备，无需用户手动配置。

#### Scenario: 首次启动自动发现
- **WHEN** 应用首次启动
- **THEN** 系统在 3 秒内发现并显示局域网内所有在线设备

#### Scenario: 新设备上线通知
- **WHEN** 新设备在局域网内启动 PaseBoard
- **THEN** 已运行的设备在 3 秒内检测到新设备并更新设备列表

### Requirement: mDNS 服务注册
系统 SHALL 使用 mDNS 协议广播自身服务信息，服务类型为 `_paseboard._tcp.local`。

#### Scenario: 端口可用时正常注册
- **WHEN** 默认端口 9527 可用
- **THEN** 系统在该端口注册 mDNS 服务并广播

#### Scenario: 端口冲突时自动降级
- **WHEN** 默认端口 9527 被占用
- **THEN** 系统自动尝试备用端口（9528, 9529...9537）并在第一个可用端口注册服务

### Requirement: 设备标识与命名
每台设备 SHALL 具有唯一标识符和可读名称，用于区分不同设备。

#### Scenario: 首次启动生成设备 ID
- **WHEN** 应用首次启动且无现有配置
- **THEN** 系统生成 UUID 作为设备 ID 并保存到配置文件

#### Scenario: 使用计算机名称作为默认名称
- **WHEN** 设备注册 mDNS 服务
- **THEN** 系统使用操作系统的计算机名称作为默认设备名称

### Requirement: 设备连接管理
系统 SHALL 自动建立与发现设备的 WebSocket 连接，维护连接状态。

#### Scenario: 发现设备后自动连接
- **WHEN** 通过 mDNS 发现新设备
- **THEN** 系统自动建立 WebSocket 连接并完成握手

#### Scenario: 设备离线检测
- **WHEN** 已连接设备超过 30 秒未响应心跳
- **THEN** 系统标记该设备为离线并在 UI 中显示离线状态

### Requirement: 设备列表显示
UI SHALL 显示所有已发现设备的列表，包括设备名称和连接状态。

#### Scenario: 显示在线设备
- **WHEN** 用户打开设备列表界面
- **THEN** UI 显示所有在线设备，带有绿色状态指示器

#### Scenario: 显示离线设备
- **WHEN** 设备离线
- **THEN** UI 将该设备标记为灰色并显示"离线"状态

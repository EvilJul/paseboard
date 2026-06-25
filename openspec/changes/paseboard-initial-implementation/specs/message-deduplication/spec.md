## ADDED Requirements

### Requirement: UUID 消息去重
系统 SHALL 记录已处理的消息 UUID，避免重复处理同一消息。

#### Scenario: 接收到新 UUID 的消息
- **WHEN** 接收到消息的 UUID 在本地记录中不存在
- **THEN** 系统处理该消息并将 UUID 记录到去重缓存

#### Scenario: 接收到重复 UUID 的消息
- **WHEN** 接收到消息的 UUID 已在本地记录中
- **THEN** 系统跳过该消息，不写入粘贴板

### Requirement: 内容哈希去重
系统 SHALL 计算并比较粘贴板内容的哈希值，避免推送重复内容。

#### Scenario: 内容哈希相同时跳过推送
- **WHEN** 当前粘贴板内容的哈希值与上次推送的哈希值相同
- **THEN** 系统不生成新消息，不推送到网络

#### Scenario: 内容哈希不同时正常推送
- **WHEN** 当前粘贴板内容的哈希值与上次不同
- **THEN** 系统生成新消息并推送

### Requirement: 双重保险机制
系统 SHALL 同时使用 UUID 去重和内容哈希去重，防止消息回环。

#### Scenario: UUID 去重失效时内容哈希兜底
- **WHEN** 某些边缘情况下 UUID 去重未生效
- **THEN** 内容哈希去重机制检测到重复内容并跳过

#### Scenario: 内容哈希去重失效时 UUID 兜底
- **WHEN** 两条不同消息的内容哈希碰撞（极小概率）
- **THEN** UUID 去重机制检测到重复消息并跳过

### Requirement: 消息来源标记
系统 SHALL 在写入粘贴板时标记消息来源，防止监听器再次推送。

#### Scenario: 标记来自网络的内容
- **WHEN** 从其他设备接收到消息并写入粘贴板
- **THEN** 系统设置"来自网络"标记，监听器检测到后跳过推送

#### Scenario: 清除来自本地的标记
- **WHEN** 用户通过 Ctrl+C 复制新内容
- **THEN** 系统清除"来自网络"标记，监听器检测到后正常推送

### Requirement: 去重缓存大小限制
UUID 去重缓存 SHALL 限制最多保留最近 1000 条记录，避免内存无限增长。

#### Scenario: 缓存未满时直接添加
- **WHEN** UUID 缓存中有 500 条记录
- **THEN** 系统直接将新 UUID 添加到缓存

#### Scenario: 缓存已满时淘汰最旧记录
- **WHEN** UUID 缓存已有 1000 条记录
- **THEN** 系统删除最早的记录后添加新 UUID

### Requirement: 回环检测
系统 SHALL 检测并阻止消息回环（设备 A → 设备 B → 设备 A）。

#### Scenario: 阻止消息回到发送方
- **WHEN** 设备 A 发送消息到设备 B，设备 B 接收后写入粘贴板
- **THEN** 设备 B 的监听器检测到"来自网络"标记，不再将该消息推送回设备 A

#### Scenario: 允许新内容正常传播
- **WHEN** 设备 B 用户复制新内容（非来自网络）
- **THEN** 设备 B 正常推送到设备 A 和其他所有设备

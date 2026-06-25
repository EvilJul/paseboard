## ADDED Requirements

### Requirement: SQLite 数据库初始化
系统 SHALL 在首次启动时创建 SQLite 数据库文件和表结构。

#### Scenario: 首次启动创建数据库
- **WHEN** 应用首次启动且数据库文件不存在
- **THEN** 系统在 `~/.paseboard/history.db` 创建数据库文件并初始化表结构

#### Scenario: 已有数据库时直接使用
- **WHEN** 数据库文件已存在
- **THEN** 系统打开现有数据库连接，不重新创建

### Requirement: 历史记录插入
系统 SHALL 在每次粘贴板内容变化时将记录插入数据库。

#### Scenario: 插入新记录
- **WHEN** 检测到粘贴板内容变化
- **THEN** 系统将内容、内容哈希、设备 ID、设备名称、时间戳、大小插入 `clipboard_history` 表

#### Scenario: 插入时计算内容哈希
- **WHEN** 插入新记录
- **THEN** 系统计算内容的 SHA256 哈希并存储在 `content_hash` 字段

### Requirement: 历史容量管理
系统 SHALL 限制历史记录最多 1000 条，超过时自动删除最旧的 100 条。

#### Scenario: 未达到容量限制时正常插入
- **WHEN** 当前历史记录数量为 500 条
- **THEN** 系统直接插入新记录，无需删除

#### Scenario: 达到容量限制时先删除再插入
- **WHEN** 当前历史记录数量已达 1000 条
- **THEN** 系统先删除最旧的 100 条记录，然后插入新记录

### Requirement: 历史查询
系统 SHALL 支持按时间倒序查询最近 N 条历史记录。

#### Scenario: 查询最近 100 条记录
- **WHEN** UI 请求加载历史记录列表
- **THEN** 系统返回按时间戳倒序排列的最近 100 条记录

#### Scenario: 查询响应时间
- **WHEN** 查询最近 100 条记录
- **THEN** 系统在 100ms 内返回结果

### Requirement: 时间戳索引
数据库 SHALL 在 `timestamp` 字段上创建索引，优化查询性能。

#### Scenario: 创建时间戳索引
- **WHEN** 数据库初始化时
- **THEN** 系统创建 `idx_timestamp` 索引在 `timestamp DESC` 上

#### Scenario: 索引加速查询
- **WHEN** 查询最近 100 条记录
- **THEN** 数据库使用索引扫描而非全表扫描

### Requirement: 内容哈希索引
数据库 SHALL 在 `content_hash` 字段上创建索引，用于快速去重检查。

#### Scenario: 创建内容哈希索引
- **WHEN** 数据库初始化时
- **THEN** 系统创建 `idx_content_hash` 索引在 `content_hash` 字段上

#### Scenario: 哈希去重查询
- **WHEN** 检查内容哈希是否已存在
- **THEN** 数据库使用索引快速查找，响应时间 < 5ms

### Requirement: 数据持久化
系统 SHALL 在应用关闭后保留历史记录数据。

#### Scenario: 应用重启后恢复历史
- **WHEN** 应用关闭后再次启动
- **THEN** 系统从数据库加载历史记录，显示在 UI 中

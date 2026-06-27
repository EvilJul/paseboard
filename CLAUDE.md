<!-- WORKFLOW-FRAMEWORK-START -->
<!--
  框架层 - 跨项目复用，定义了完整的工作流编排。
  复制到新项目时，此区块保持不变。
  如需调整流程，在此区块内修改。
-->

# CLAUDE.md

## 文件体系说明

本项目使用三层 CLAUDE.md 体系：

```
~/.claude/CLAUDE.md          ← 全局层：个人偏好、语言约定、Obsidian 规范、通用禁区
模板文件（本文件）            ← 模板层：流水线编排 + 阶段信号 + 内容层占位
项目根/CLAUDE.md             ← 实例层：复制本模板，内容层已填充为具体项目指令
```

**本文件与全局文件的分工**：全局文件管"怎么存、怎么说、什么绝对不能做"；本文件管"流水线怎么跑、这个项目的技术栈怎么用"。

---

## 角色定义

你是本项目的**主协调 agent**。你的核心职责是调度流水线、检查产出、决定流转。

**铁律：你不直接编写业务代码。** 所有编码实现必须通过子 agent 完成，子 agent 使用 `Agent` 工具创建（`isolation: "worktree"`）。

你是一个导演，不是一个演员。

---

## 开发流水线

本项目严格遵循以下 9 步串行流水线，不可跳过、不可合并：

```
office-hours → plan-eng-review → [生成 CLAUDE.md] → openspec → claude code → review → cso → qa → ship
```

### 阶段总览

| # | 阶段 | 触发方式 | 产出 | 产出位置 |
|---|------|---------|------|---------|
| 1 | 想法验证 | `/office-hours` | 产品方向文档 | Obsidian `项目/[项目名]/00-项目概览.md` |
| 2 | 架构评审 | `/plan-eng-review` | 技术决策记录 | Obsidian `项目/[项目名]/技术方案/{日期}-架构决策.md` |
| 2.5 | CLAUDE.md 生成 | 主 agent 填充 | 项目级 AI 指令 | **项目根** `CLAUDE.md` |
| 3 | 规范拆解 | `/opsx:propose` | OpenSpec 变更提案 | **项目根** `openspec/changes/{name}/` |
| 4 | 编码实现 | claude code 子 agent | 源代码 | **项目根** `src/`、`tests/` |
| 5 | 代码评审 | `/review` | 评审记录 | Obsidian `项目/[项目名]/问题记录/{日期}-代码评审.md` |
| 6 | 安全评审 | `/cso` | 安全报告 | Obsidian `项目/[项目名]/问题记录/{日期}-安全审计.md` |
| 7 | 测试验证 | `/qa` | 测试报告 | Obsidian `项目/[项目名]/问题记录/{日期}-测试报告.md` |
| 8 | 发布上线 | `/ship` | PR、CHANGELOG | GitHub + **项目根** `CHANGELOG.md` |

### 阶段间规则

1. **产出即输入** — 上一阶段的产出文件是下一阶段的输入，不得跳过
2. **红灯即停** — Review、CSO、QA 任意一个不通过，必须回退到编码阶段（#4），不可绕过
3. **汇报确认** — 每个阶段完成后，向用户汇报：产出物、关键决策、是否建议继续
4. **独立任务** — 编码阶段中，每个独立功能点创建一个子 agent，不要把所有任务塞进同一个 agent
5. **用户在环** — 任何阶段出现分歧，列出选项及利弊，让用户决策，不要自己拍板

---

## 阶段信号

每个阶段启动/通过/回退时，主 agent **必须**输出对应的信号 banner。这是流水线的"路标"。

### 阶段启动 banner

```
╔══════════════════════════════════════╗
║  💡 OFFICE-HOURS                    ║
║  ── 想法验证 → 方向确认 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  🏗️ PLAN-ENG-REVIEW                  ║
║  ── 技术选型 → 架构设计 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  📋 CLAUDE.md 生成                  ║
║  ── 技术决策 → 项目指令 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  📐 OPENSPEC                        ║
║  ── 规范驱动 → 任务拆解 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  ⚙️ CLAUDE CODE                      ║
║  ── 子 agent 编码 → 代码产出 ──      ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  🔍 REVIEW                          ║
║  ── 代码 diff → 质量评审 ──          ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  🛡️ CSO                              ║
║  ── 安全审计 → 漏洞扫描 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  🧪 QA                              ║
║  ── 功能验证 → 测试报告 ──           ║
╚══════════════════════════════════════╝
```

```
╔══════════════════════════════════════╗
║  🚢 SHIP                            ║
║  ── 合并分支 → 发布上线 ──           ║
╚══════════════════════════════════════╝
```

### 阶段转换

```
  ✅ {上游阶段} 完成 ─────▶ {下游emoji} {下游阶段} 启动
```

### 回退信号

```
  ❌ REVIEW 不通过 ─────▶ 🔄 回退 CLAUDE CODE（阶段 #4）
  ❌ CSO 不通过    ─────▶ 🔄 回退 CLAUDE CODE（阶段 #4）
  ❌ QA 不通过     ─────▶ 🔄 回退 CLAUDE CODE（阶段 #4）
```

### 回退规则

无论哪个评审阶段（Review / CSO / QA）发现问题，统一回退路径：

```
claude code ──→ review ──→ cso ──→ qa ──→ ship
    ↑              │         │       │
    └────── 不通过 ┘         │       │
    └─────────── 不通过 ─────┘       │
    └─────────────── 不通过 ─────────┘
```

- 回退后，主 agent 汇总所有不通过的原因，交给子 agent 集中修复
- 修复完成后，从 Review 重新开始走完整个评审链（Review → CSO → QA），不能跳过
- 局部小修复（单文件改动）可以跳过 CSO，由主 agent 判断

### 流水线完成

```
══════════════════════════════════════
  🏁 流水线完成 ── 全部 8 阶段通过
══════════════════════════════════════
```

---

## Skill 调用规范

### gstack skills（阶段 1-2, 5-8）

- 通过 `/` 命令调用
- 调用前确保：上一阶段产出已就绪、用户已确认进入该阶段
- 调用时输出对应的**阶段启动 banner**

### Claude Code 子 agent（阶段 4）

编码阶段使用 `Agent` 工具，每个独立任务一个子 agent：

```
Agent(
  description: "{简短任务描述}",
  prompt: "{完整任务说明，包含 OpenSpec tasks.md 中的对应任务、输入规范文件路径、输出要求}",
  isolation: "worktree",
  subagent_type: "general-purpose"
)
```

使用规则：
- **一个子 agent = 一个独立任务**，openspec tasks.md 中的一个 `[ ]` 项
- **使用 worktree 隔离**，防止并行子 agent 的文件冲突
- **子 agent 完成后**，主 agent 检查：产物是否与 openspec 一致、能否通过类型检查
- **全部子 agent 完成后**，汇总产出，汇报用户，询问是否进入 Review

### OpenSpec 集成（阶段 3）

- 命令：`/opsx:propose`
- 输入：阶段 1 的产品方向 + 阶段 2 的技术决策
- 产出：`openspec/changes/{name}/`（proposal.md + specs/ + tasks.md）
- tasks.md 中的任务列表 = 阶段 4 子 agent 的拆分依据

### 知识提示 banner

执行辅助性 Bash 命令前，输出知识 tip：

```
┌─ 💡 ──────────────────────────────────────
│  {这一步在解决什么问题，一句话说清"为什么"}
└───────────────────────────────────────────
```

不需要 tip 的场景：阶段 skill 调用（已有 banner）、子 agent 创建（Agent 工具本身已说明意图）。

### Obsidian 写入

所有知识类产出统一写入 Obsidian 个人知识库，具体路径和写入方式遵循全局 `~/.claude/CLAUDE.md` 中「项目记录管理」章节。使用 `/obsidian-markdown` skill 创建记录。

---

## 项目目录结构

项目根目录只保留工程产物，知识资产全部存 Obsidian：

```
项目根/
├── CLAUDE.md            # 项目级 AI 指令（本文件）
├── openspec/            # OpenSpec 规范（skill 自动管理）
│   ├── changes/         #   变更提案
│   └── specs/           #   主规范
├── src/                 # 源代码
├── tests/               # 测试代码
└── CHANGELOG.md         # 变更日志（ship 自动生成）
```

不含 `docs/` 目录。所有文档（决策、评审、报告）存入 Obsidian。

---

## 语言约定

参见全局 `~/.claude/CLAUDE.md`，不在此重复。

---

## 行为禁区

以下为项目级补充禁区，通用禁区参见全局 `~/.claude/CLAUDE.md`。

### 流程禁区
- **禁止**跳过流水线中的任何阶段
- **禁止**在上一阶段未确认通过时启动下一阶段
- **禁止**在 Review / CSO / QA 不通过时自行修复后直接标记通过

### 代码禁区
- **禁止**引入 plan-eng-review 中未决定的新第三方依赖
- **禁止**删除或修改已有的代码注释
- **禁止**擅自将同步函数改为异步、或改变已有 API 的函数签名

<!-- WORKFLOW-FRAMEWORK-END -->

<!-- ═══════════════════════════════════════════════════════════ -->
<!--                     项目内容层                              -->
<!--                                                           -->
<!--  下方是项目级的"灵魂 + 铁律 + 配方"。                      -->
<!--  由主 agent 在 plan-eng-review 完成后，                    -->
<!--  读取技术决策记录，自动填充所有 [待填充] 和空白章节。       -->
<!-- ═══════════════════════════════════════════════════════════ -->

<!-- PROJECT-CONTENT-START -->

## 项目灵魂

我们正在做 **PaseBoard**，一个跨平台局域网粘贴板同步工具。

**目标用户：** 拥有多台设备（电脑、平板）的开发者、写作者、设计师，需要在设备间频繁复制粘贴内容。

**产品气质：** 
- **零配置，开箱即用** — 不需要注册账号、不需要配置服务器，启动即自动发现局域网内其他设备
- **实时同步，低延迟** — 复制后 1 秒内出现在其他设备
- **轻量透明** — 后台运行，不干扰主要工作流，内存占用 < 50MB

**核心交互原则：**
- 无感知同步：用户只需正常复制粘贴，无需额外操作
- 系统托盘常驻：双击托盘图标打开历史记录，右键退出
- 历史记录可查：最近 1000 条粘贴板历史，随时回溯

---

## 技术栈与使用铁律

### 桌面框架
- **框架：Tauri v1**
  - 使用 Tauri 的跨平台能力，统一 Windows、macOS、Linux 代码
  - 禁止直接调用平台特定 API，必须通过 Tauri 插件抽象层
  
- **前端：HTML + CSS + JavaScript（原生）**
  - 不使用 React/Vue 等框架（保持轻量）
  - 禁止引入前端打包工具（Vite/Webpack），直接加载 HTML

### 后端语言
- **Rust 1.70+**
  - 所有后端逻辑用 Rust 实现
  - 禁止使用 unsafe 代码块（除非性能关键且经过评审）
  - 使用 `#![warn(clippy::all)]` 启用全部 Clippy 检查

### 异步运行时
- **Tokio**（通过 Tauri 依赖自动引入）
  - 所有网络 I/O 必须使用 async/await
  - 禁止使用 `block_on` 阻塞异步运行时
  - 长时间运行的任务用 `tokio::spawn` 创建独立 task

### 网络层
- **设备发现：mdns-sd crate**
  - 服务类型：`_paseboard._tcp.local`
  - mDNS 广播间隔：5 秒
  - 禁止手动实现 mDNS 协议，必须用 `mdns-sd` crate

- **实时通信：WebSocket (tokio-tungstenite)**
  - 每对设备间维护一条双向 WebSocket 连接
  - 心跳间隔：30 秒
  - 重连策略：指数退避，最多重试 3 次
  - 消息格式：JSON（使用 serde_json）

### 粘贴板操作
- **tauri-plugin-clipboard-manager**
  - 监听粘贴板：500ms 轮询间隔
  - 统一通过 tauri-plugin-clipboard-manager 访问粘贴板，禁止直接使用 arboard 等系统 API
  - 标记消息来源：写入时设置标记，避免消息回环

### 数据存储
- **SQLite（rusqlite crate）**
  - 数据库文件：`~/.paseboard/history.db`
  - 所有 SQL 语句必须使用参数化查询（防止 SQL 注入）
  - 禁止使用 ORM，直接写 SQL
  - 事务管理：历史插入 + 容量清理必须在同一事务内

### 错误处理
- **混合模式：**
  - 核心模块（`network/`, `clipboard/`）定义自定义 Error enum（使用 `thiserror`）
  - 应用层（`app.rs`, `main.rs`）使用 `anyhow::Result` 简化错误传播
  - 所有错误必须包含上下文信息（使用 `.context()` 方法）

### 日志
- **log + env_logger**
  - 日志级别：`RUST_LOG=info`（生产环境）
  - 禁止在日志中记录粘贴板内容原文（隐私保护）
  - 错误日志必须包含完整调用栈

### 依赖管理
- **精确版本锁定**
  - `Cargo.toml` 中所有依赖使用精确版本（`version = "1.35.0"`）
  - 禁止使用通配符版本（`version = "1.*"`）
  - 定期检查依赖安全漏洞：`cargo audit`

---

## 代码生成配方

### Rust 模块配方

每个模块文件结构顺序：

```rust
// 1. 标准库导入
use std::collections::HashMap;
use std::sync::Arc;

// 2. 第三方 crate 导入
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

// 3. 本地模块导入
use crate::network::message::Message;
use crate::utils::error::NetworkError;

// 4. 类型定义
pub struct DeviceManager {
    devices: Arc<RwLock<HashMap<String, Device>>>,
}

// 5. Trait 实现
impl DeviceManager {
    // 构造函数在最前面
    pub fn new() -> Self { ... }
    
    // 公有方法
    pub fn add_device(&self, device: Device) { ... }
    
    // 私有方法
    fn internal_cleanup(&self) { ... }
}

// 6. 单元测试（在同一文件末尾）
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_add_device() { ... }
}
```

### 异步函数配方

```rust
// ✅ 推荐：使用 ? 操作符传播错误
pub async fn send_message(&self, msg: Message) -> Result<()> {
    let bytes = serde_json::to_vec(&msg)?;
    self.socket.send(bytes).await?;
    Ok(())
}

// ❌ 禁止：手动 match 错误（冗长）
pub async fn send_message(&self, msg: Message) -> Result<()> {
    match serde_json::to_vec(&msg) {
        Ok(bytes) => { ... }
        Err(e) => Err(e.into()),
    }
}
```

### 错误处理配方

核心模块错误定义（`utils/error.rs`）：

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("连接失败: {0}")]
    ConnectionFailed(String),
    
    #[error("消息解析失败: {0}")]
    MessageParseFailed(String),
    
    #[error("心跳超时")]
    HeartbeatTimeout,
    
    #[error("内容超过大小限制: {size} bytes")]
    ContentTooLarge { size: usize },
}

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("粘贴板锁定")]
    ClipboardLocked,
    
    #[error("内容超过大小限制: {0} bytes")]
    ContentTooLarge(usize),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("数据库操作失败: {0}")]
    DatabaseError(#[from] rusqlite::Error),
}
```

### 数据库查询配方

所有 SQL 放在 `clipboard/storage.rs`：

```rust
// ✅ 推荐：参数化查询 + 索引
pub fn query_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>> {
    let mut stmt = self.conn.prepare(
        "SELECT * FROM clipboard_history 
         ORDER BY timestamp DESC 
         LIMIT ?"
    )?;
    
    let records = stmt.query_map([limit], |row| {
        Ok(ClipboardRecord {
            id: row.get(0)?,
            content: row.get(1)?,
            device_name: row.get(3)?,
            timestamp: row.get(4)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;
    
    Ok(records)
}

// ❌ 禁止：字符串拼接 SQL（SQL 注入风险）
pub fn query_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>> {
    let sql = format!("SELECT * FROM clipboard_history LIMIT {}", limit);
    // 危险！
}
```

### WebSocket 消息广播配方

```rust
// ✅ 推荐：序列化一次 + 并发发送
pub async fn broadcast_message(&self, message: &Message) -> Result<()> {
    // 序列化一次
    let bytes = serde_json::to_vec(message)?;
    
    // 并发发送给所有设备
    let send_futures: Vec<_> = self.connected_devices
        .read()
        .await
        .values()
        .map(|device| device.send(bytes.clone()))
        .collect();
    
    let results = futures::future::join_all(send_futures).await;
    
    // 统计成功/失败
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    log::info!("消息已发送到 /{} 台设备", success_count, results.len());
    
    Ok(())
}
```

---

## 项目特定禁区

### 安全禁区
- **禁止**在日志中记录粘贴板内容原文（隐私保护）
- **禁止**将粘贴板内容上传到任何云端服务
- **禁止**在 WebSocket 握手时跳过设备 ID 验证

### 性能禁区
- **禁止**在粘贴板监听线程中执行阻塞操作（会卡住轮询）
- **禁止**在消息广播时对每个设备重复序列化（必须序列化一次后复用）
- **禁止**在没有索引的情况下查询历史记录（必须创建 `idx_timestamp` 索引）

### 数据禁区
- **禁止**删除或修改 `clipboard_history` 表的 Schema（数据迁移必须通过 migration 脚本）
- **禁止**在历史记录达到 1000 条时继续插入而不清理（必须先删除最旧的 100 条）

### 网络禁区
- **禁止**手动实现 mDNS 协议（必须使用 `mdns-sd` crate）
- **禁止**在 WebSocket 连接失败后无限重试（最多重试 3 次）
- **禁止**发送超过 10MB 的粘贴板内容（必须在发送前检查大小）

---

## 常用命令

| 场景 | 命令 |
|------|------|
| 启动开发 | `cargo tauri dev` |
| 类型检查 | `cargo check` |
| 代码检查 | `cargo clippy --all-targets --all-features` |
| 单元测试 | `cargo test --lib` |
| 集成测试 | `cargo test --test '*'` |
| 性能基准测试 | `cargo bench` |
| 安全审计 | `cargo audit` |
| 打包构建（Universal，推荐） | `cargo tauri build --target universal-apple-darwin` |
| 打包构建（仅当前平台） | `cargo tauri build` |

---

## 当前任务

- **正在做**：项目初始化，生成项目级 CLAUDE.md
- **紧急性**：中
- **当前阶段**：阶段 2.5（CLAUDE.md 生成）
- **已完成文件**：
  - Obsidian 技术决策记录（4 个文件）
  - 本项目级 CLAUDE.md
- **注意事项**：
  - 下一步运行 `/opsx:propose` 生成 OpenSpec 规范
  - 或运行 `/gstack-design-consultation` 设计视觉系统
  - 或直接开始编码实现

<!-- PROJECT-CONTENT-END -->

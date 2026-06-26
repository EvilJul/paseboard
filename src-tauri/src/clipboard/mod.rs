// 粘贴板管理模块
//
// 职责：
// - 粘贴板内容监听（轮询）
// - 粘贴板内容写入
// - 历史记录存储（SQLite）
// - 内容去重（SHA256 哈希）
// - 消息去重服务（UUID + 内容哈希）

pub mod monitor;
pub mod writer;
pub mod storage;
pub mod dedup;

// 重新导出主要类型
pub use monitor::{ClipboardMonitor, ClipboardChange};
pub use writer::ClipboardWriter;
pub use storage::{HistoryStorage, HistoryItem};
pub use dedup::DeduplicationService;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use serde::Serialize;

/// 日志条目
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

/// 日志环形缓存
#[derive(Clone)]
pub struct LogBuffer {
    buffer: Arc<Mutex<VecDeque<LogEntry>>>,
    max_entries: usize,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(max_entries))),
            max_entries,
        }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.max_entries {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        let buf = self.buffer.lock().unwrap();
        buf.iter().cloned().collect()
    }
}

/// 复合 Logger：同时写 env_logger（stderr）和 LogBuffer（内存缓存）
pub struct CompositeLogger {
    buffer: LogBuffer,
}

impl CompositeLogger {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl log::Log for CompositeLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        let entry = LogEntry {
            level: record.level().to_string(),
            message: format!("{}", record.args()),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // 存到内存缓存
        self.buffer.push(entry);

        // 同时输出到 stderr（开发者模式仍保留终端输出）
        eprintln!("[{}] {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;

    #[test]
    fn test_log_buffer_push_and_snapshot() {
        let buffer = LogBuffer::new(10);
        assert!(buffer.snapshot().is_empty());

        buffer.push(LogEntry {
            level: "INFO".into(),
            message: "test message".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
        });

        let snap = buffer.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].level, "INFO");
        assert_eq!(snap[0].message, "test message");
    }

    #[test]
    fn test_log_buffer_capacity() {
        let buffer = LogBuffer::new(3);
        for i in 0..5 {
            buffer.push(LogEntry {
                level: "INFO".into(),
                message: format!("msg {}", i),
                timestamp: String::new(),
            });
        }

        let snap = buffer.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].message, "msg 2");
        assert_eq!(snap[2].message, "msg 4");
    }

    #[test]
    fn test_log_buffer_clone_shares_state() {
        let buffer = LogBuffer::new(10);
        let cloned = buffer.clone();

        buffer.push(LogEntry {
            level: "WARN".into(),
            message: "shared".into(),
            timestamp: String::new(),
        });

        // 克隆体应看到同一数据
        assert_eq!(cloned.snapshot().len(), 1);
        assert_eq!(cloned.snapshot()[0].message, "shared");
    }

    #[test]
    fn test_composite_logger_enabled() {
        let buffer = LogBuffer::new(10);
        let logger = CompositeLogger::new(buffer.clone());

        let info_meta = log::Metadata::builder()
            .level(log::Level::Info)
            .target("test")
            .build();
        let debug_meta = log::Metadata::builder()
            .level(log::Level::Debug)
            .target("test")
            .build();

        assert!(logger.enabled(&info_meta));
        assert!(!logger.enabled(&debug_meta));
    }
}

// 核心错误类型定义
//
// 职责：
// - 定义网络、剪贴板、存储三大模块的错误类型
// - 使用 thiserror 实现 std::error::Error 自动派生
// - 提供 From 实现，支持错误自动转换

use thiserror::Error;

/// 网络模块错误
#[derive(Error, Debug)]
pub enum NetworkError {
    /// 连接失败（对方不可达、端口未开放等）
    #[error("连接失败: {0}")]
    ConnectionFailed(String),

    /// 消息解析失败（协议格式错误、JSON 解析失败等）
    #[error("消息解析失败: {0}")]
    MessageParseFailed(String),

    /// 心跳超时（对端在超时时间内未响应心跳）
    #[error("心跳超时: 对端 {peer_id} 在 {timeout_secs} 秒内未响应")]
    HeartbeatTimeout {
        peer_id: String,
        timeout_secs: u64,
    },

    /// 传输内容超过限制
    #[error("内容过大: {size} 字节超过限制 {limit} 字节")]
    ContentTooLarge {
        size: u64,
        limit: u64,
    },
}

/// 剪贴板模块错误
#[derive(Error, Debug)]
pub enum ClipboardError {
    /// 剪贴板被其他进程锁定或无法访问
    #[error("剪贴板被锁定: {0}")]
    ClipboardLocked(String),

    /// 写入/读取内容超过剪贴板能力限制
    #[error("剪贴板内容过大: {0}")]
    ContentTooLarge(String),
}

/// 存储模块错误
#[derive(Error, Debug)]
pub enum StorageError {
    /// 数据库操作失败（自动从 rusqlite::Error 转换）
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] rusqlite::Error),
}
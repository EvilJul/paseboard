// 消息协议定义
//
// 职责：
// - 定义 WebSocket 消息结构体
// - 实现 JSON 序列化/反序列化
// - 提供消息构造方法

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// WebSocket 消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum MessageType {
    /// 粘贴板内容同步消息
    #[serde(rename = "clipboard")]
    Clipboard {
        /// 消息唯一标识（用于去重）
        uuid: String,
        /// 粘贴板内容
        content: String,
        /// 来源设备 ID
        device_id: String,
        /// Unix 时间戳（秒）
        timestamp: i64,
    },

    /// 心跳消息（用于保活和超时检测）
    #[serde(rename = "heartbeat")]
    Heartbeat {
        /// 发送设备 ID
        device_id: String,
        /// Unix 时间戳（秒）
        timestamp: i64,
    },

    /// 心跳响应消息
    #[serde(rename = "heartbeat_ack")]
    HeartbeatAck {
        /// 响应设备 ID
        device_id: String,
        /// Unix 时间戳（秒）
        timestamp: i64,
    },
}

/// WebSocket 消息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    #[serde(flatten)]
    pub msg_type: MessageType,
}

impl Message {
    /// 创建粘贴板同步消息
    pub fn new_clipboard(content: String, device_id: String) -> Self {
        let uuid = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        Self {
            msg_type: MessageType::Clipboard {
                uuid,
                content,
                device_id,
                timestamp,
            },
        }
    }

    /// 创建心跳消息
    pub fn new_heartbeat(device_id: String) -> Self {
        let timestamp = chrono::Utc::now().timestamp();

        Self {
            msg_type: MessageType::Heartbeat {
                device_id,
                timestamp,
            },
        }
    }

    /// 创建心跳响应消息
    pub fn new_heartbeat_ack(device_id: String) -> Self {
        let timestamp = chrono::Utc::now().timestamp();

        Self {
            msg_type: MessageType::HeartbeatAck {
                device_id,
                timestamp,
            },
        }
    }

    /// 获取消息 UUID（仅 Clipboard 类型有效）
    pub fn uuid(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::Clipboard { uuid, .. } => Some(uuid),
            _ => None,
        }
    }

    /// 获取消息来源设备 ID
    pub fn device_id(&self) -> &str {
        match &self.msg_type {
            MessageType::Clipboard { device_id, .. }
            | MessageType::Heartbeat { device_id, .. }
            | MessageType::HeartbeatAck { device_id, .. } => device_id,
        }
    }

    /// 获取消息时间戳
    pub fn timestamp(&self) -> i64 {
        match &self.msg_type {
            MessageType::Clipboard { timestamp, .. }
            | MessageType::Heartbeat { timestamp, .. }
            | MessageType::HeartbeatAck { timestamp, .. } => *timestamp,
        }
    }

    /// 获取粘贴板内容（仅 Clipboard 类型有效）
    pub fn content(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::Clipboard { content, .. } => Some(content),
            _ => None,
        }
    }

    /// 判断是否为心跳消息
    pub fn is_heartbeat(&self) -> bool {
        matches!(self.msg_type, MessageType::Heartbeat { .. })
    }

    /// 判断是否为心跳响应消息
    pub fn is_heartbeat_ack(&self) -> bool {
        matches!(self.msg_type, MessageType::HeartbeatAck { .. })
    }

    /// 判断是否为粘贴板消息
    pub fn is_clipboard(&self) -> bool {
        matches!(self.msg_type, MessageType::Clipboard { .. })
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 字符串反序列化
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 获取消息大小（字节数）
    pub fn size(&self) -> usize {
        // 估算消息大小（JSON 序列化后的大小）
        match &self.msg_type {
            MessageType::Clipboard { uuid, content, device_id, .. } => {
                // 估算 JSON 结构开销 + 实际内容
                uuid.len() + content.len() + device_id.len() + 100
            }
            MessageType::Heartbeat { device_id, .. }
            | MessageType::HeartbeatAck { device_id, .. } => {
                device_id.len() + 50
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_message_creation() {
        let msg = Message::new_clipboard(
            "Hello, World!".to_string(),
            "device-123".to_string(),
        );

        assert!(msg.is_clipboard());
        assert!(!msg.is_heartbeat());
        assert_eq!(msg.device_id(), "device-123");
        assert_eq!(msg.content(), Some("Hello, World!"));
        assert!(msg.uuid().is_some());
    }

    #[test]
    fn test_heartbeat_message_creation() {
        let msg = Message::new_heartbeat("device-456".to_string());

        assert!(msg.is_heartbeat());
        assert!(!msg.is_clipboard());
        assert_eq!(msg.device_id(), "device-456");
        assert_eq!(msg.content(), None);
    }

    #[test]
    fn test_heartbeat_ack_message_creation() {
        let msg = Message::new_heartbeat_ack("device-789".to_string());

        assert!(msg.is_heartbeat_ack());
        assert!(!msg.is_clipboard());
        assert_eq!(msg.device_id(), "device-789");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::new_clipboard(
            "Test content".to_string(),
            "device-abc".to_string(),
        );

        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"clipboard\""));
        assert!(json.contains("\"content\":\"Test content\""));
        assert!(json.contains("\"device_id\":\"device-abc\""));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{
            "type": "clipboard",
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "content": "Test",
            "device_id": "device-1",
            "timestamp": 1703520000
        }"#;

        let msg = Message::from_json(json).unwrap();
        assert!(msg.is_clipboard());
        assert_eq!(msg.content(), Some("Test"));
        assert_eq!(msg.device_id(), "device-1");
    }

    #[test]
    fn test_message_size() {
        let msg = Message::new_clipboard(
            "A".repeat(1000),
            "device-1".to_string(),
        );

        let size = msg.size();
        assert!(size > 1000); // 内容 + 元数据
    }
}

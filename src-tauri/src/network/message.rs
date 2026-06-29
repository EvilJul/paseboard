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

    /// 配对请求消息
    #[serde(rename = "pairing_request")]
    PairingRequest {
        /// 来源设备 ID
        device_id: String,
        /// 来源设备名称
        device_name: String,
        /// 公钥指纹（SHA256 前 16 字符）
        device_pk_fingerprint: String,
        /// Unix 时间戳（秒）
        timestamp: i64,
    },

    /// 配对响应消息
    #[serde(rename = "pairing_response")]
    PairingResponse {
        /// 响应设备 ID
        device_id: String,
        /// 是否接受配对
        accepted: bool,
        /// 拒绝原因（可选）
        reason: Option<String>,
        /// Unix 时间戳（秒）
        timestamp: i64,
    },

    /// 加密消息（非对称加密后的载荷）
    #[serde(rename = "encrypted")]
    Encrypted {
        /// 12 字节随机 nonce
        nonce: Vec<u8>,
        /// AES-256-GCM 加密后的密文
        ciphertext: Vec<u8>,
        /// 发送方 X25519 公钥（32 字节）
        public_key: Vec<u8>,
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

    /// 创建配对请求消息
    pub fn new_pairing_request(device_id: String, device_name: String, device_pk_fingerprint: String) -> Self {
        let timestamp = chrono::Utc::now().timestamp();
        Self {
            msg_type: MessageType::PairingRequest {
                device_id,
                device_name,
                device_pk_fingerprint,
                timestamp,
            },
        }
    }

    /// 创建配对响应消息
    pub fn new_pairing_response(device_id: String, accepted: bool, reason: Option<String>) -> Self {
        let timestamp = chrono::Utc::now().timestamp();
        Self {
            msg_type: MessageType::PairingResponse {
                device_id,
                accepted,
                reason,
                timestamp,
            },
        }
    }

    /// 获取消息 UUID（仅 Clipboard 类型有效）
    pub fn uuid(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::Clipboard { uuid, .. } => Some(uuid),
            MessageType::Encrypted { .. } => None,
            _ => None,
        }
    }

    /// 获取配对请求的设备名称（仅 PairingRequest 有效）
    pub fn pairing_device_name(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::PairingRequest { device_name, .. } => Some(device_name),
            _ => None,
        }
    }

    /// 获取配对请求的公钥指纹（仅 PairingRequest 有效）
    pub fn pairing_pk_fingerprint(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::PairingRequest { device_pk_fingerprint, .. } => Some(device_pk_fingerprint),
            _ => None,
        }
    }

    /// 获取配对响应的 accepted 状态（仅 PairingResponse 有效）
    pub fn pairing_accepted(&self) -> Option<bool> {
        match &self.msg_type {
            MessageType::PairingResponse { accepted, .. } => Some(*accepted),
            _ => None,
        }
    }

    /// 获取配对拒绝原因（仅 PairingResponse 有效）
    pub fn pairing_reason(&self) -> Option<Option<&str>> {
        match &self.msg_type {
            MessageType::PairingResponse { reason, .. } => Some(reason.as_deref()),
            _ => None,
        }
    }

    /// 获取消息来源设备 ID
    pub fn device_id(&self) -> &str {
        match &self.msg_type {
            MessageType::Clipboard { device_id, .. }
            | MessageType::Heartbeat { device_id, .. }
            | MessageType::HeartbeatAck { device_id, .. }
            | MessageType::PairingRequest { device_id, .. }
            | MessageType::PairingResponse { device_id, .. } => device_id,
            MessageType::Encrypted { .. } => "unknown",
        }
    }

    /// 获取消息时间戳
    pub fn timestamp(&self) -> i64 {
        match &self.msg_type {
            MessageType::Clipboard { timestamp, .. }
            | MessageType::Heartbeat { timestamp, .. }
            | MessageType::HeartbeatAck { timestamp, .. }
            | MessageType::PairingRequest { timestamp, .. }
            | MessageType::PairingResponse { timestamp, .. } => *timestamp,
            MessageType::Encrypted { .. } => 0,
        }
    }

    /// 获取粘贴板内容（仅 Clipboard 类型有效）
    pub fn content(&self) -> Option<&str> {
        match &self.msg_type {
            MessageType::Clipboard { content, .. } => Some(content),
            MessageType::Encrypted { .. } => None,
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

    /// 判断是否为配对请求
    pub fn is_pairing_request(&self) -> bool {
        matches!(self.msg_type, MessageType::PairingRequest { .. })
    }

    /// 判断是否为配对响应
    pub fn is_pairing_response(&self) -> bool {
        matches!(self.msg_type, MessageType::PairingResponse { .. })
    }

    /// 判断是否为配对消息（请求或响应）
    pub fn is_pairing(&self) -> bool {
        matches!(self.msg_type, MessageType::PairingRequest { .. } | MessageType::PairingResponse { .. })
    }

    /// 判断是否为加密消息
    pub fn is_encrypted(&self) -> bool {
        matches!(self.msg_type, MessageType::Encrypted { .. })
    }

    /// 创建加密消息
    pub fn new_encrypted(
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
        public_key: Vec<u8>,
    ) -> Self {
        Self {
            msg_type: MessageType::Encrypted {
                nonce,
                ciphertext,
                public_key,
            },
        }
    }

    /// 获取加密消息的载荷字段
    pub fn encrypted_payload(&self) -> Option<(&[u8], &[u8], &[u8])> {
        match &self.msg_type {
            MessageType::Encrypted {
                nonce,
                ciphertext,
                public_key,
            } => Some((nonce, ciphertext, public_key)),
            _ => None,
        }
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
            MessageType::PairingRequest { device_id, device_name, device_pk_fingerprint, .. } => {
                device_id.len() + device_name.len() + device_pk_fingerprint.len() + 80
            }
            MessageType::PairingResponse { device_id, reason, .. } => {
                device_id.len() + reason.as_ref().map(|r| r.len()).unwrap_or(0) + 80
            }
            MessageType::Encrypted {
                nonce,
                ciphertext,
                public_key,
                ..
            } => nonce.len() + ciphertext.len() + public_key.len() + 100,
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
    fn test_pairing_request_creation() {
        let msg = Message::new_pairing_request(
            "device-1".to_string(),
            "My MacBook".to_string(),
            "a1b2c3d4e5f6a7b8".to_string(),
        );

        assert!(msg.is_pairing_request());
        assert!(!msg.is_pairing_response());
        assert!(msg.is_pairing());
        assert_eq!(msg.device_id(), "device-1");
        assert_eq!(msg.pairing_device_name(), Some("My MacBook"));
        assert_eq!(msg.pairing_pk_fingerprint(), Some("a1b2c3d4e5f6a7b8"));
        assert_eq!(msg.content(), None);
    }

    #[test]
    fn test_pairing_response_creation() {
        let accepted = Message::new_pairing_response(
            "device-2".to_string(),
            true,
            None,
        );
        assert!(accepted.is_pairing_response());
        assert!(accepted.is_pairing());
        assert_eq!(accepted.device_id(), "device-2");
        assert_eq!(accepted.pairing_accepted(), Some(true));
        assert_eq!(accepted.pairing_reason(), Some(None));

        let rejected = Message::new_pairing_response(
            "device-2".to_string(),
            false,
            Some("cooldown".to_string()),
        );
        assert!(rejected.is_pairing_response());
        assert_eq!(rejected.pairing_accepted(), Some(false));
        assert_eq!(rejected.pairing_reason(), Some(Some("cooldown")));
    }

    #[test]
    fn test_pairing_message_serialization() {
        let msg = Message::new_pairing_request(
            "device-1".to_string(),
            "My MacBook".to_string(),
            "a1b2c3d4e5f6a7b8".to_string(),
        );

        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"pairing_request\""));
        assert!(json.contains("\"device_name\":\"My MacBook\""));
        assert!(json.contains("\"device_pk_fingerprint\":\"a1b2c3d4e5f6a7b8\""));
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

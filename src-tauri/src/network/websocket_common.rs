// WebSocket 共享逻辑
//
// 职责：
// - 消息编解码
// - 心跳检测逻辑
// - 内容大小限制检查
// - 重连策略计算

use crate::utils::error::NetworkError;
use super::crypto::{CryptoSession, CryptoTransport, EncryptedPayload};
use super::message::Message;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// 内容大小上限（10MB）
pub const MAX_CONTENT_SIZE: usize = 10 * 1024 * 1024;

/// 心跳间隔（30 秒）
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// 心跳超时（60 秒）
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 60;

/// 最大重连次数
pub const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// 初始重连延迟（2 秒）
pub const INITIAL_RECONNECT_DELAY_SECS: u64 = 2;

/// 检查消息内容大小是否超过限制
///
/// # 参数
/// - `msg`: 待检查的消息
///
/// # 返回
/// - `Ok(())`: 大小未超限
/// - `Err(NetworkError::ContentTooLarge)`: 大小超限
pub fn check_message_size(msg: &Message) -> Result<(), NetworkError> {
    let size = msg.size();
    if size > MAX_CONTENT_SIZE {
        return Err(NetworkError::ContentTooLarge {
            size: size as u64,
            limit: MAX_CONTENT_SIZE as u64,
        });
    }
    Ok(())
}

/// 将应用层 Message 编码为 WebSocket 文本消息
///
/// # 参数
/// - `msg`: 应用层消息
///
/// # 返回
/// - `Ok(WsMessage)`: 编码后的 WebSocket 消息
/// - `Err(NetworkError)`: 序列化失败或大小超限
pub fn encode_message(msg: &Message) -> Result<WsMessage, NetworkError> {
    // 检查大小限制
    check_message_size(msg)?;

    // 序列化为 JSON
    let json = msg
        .to_json()
        .map_err(|e| NetworkError::MessageParseFailed(format!("序列化失败: {}", e)))?;

    Ok(WsMessage::Text(json))
}

/// 将 WebSocket 消息解码为应用层 Message
///
/// # 参数
/// - `ws_msg`: WebSocket 消息
///
/// # 返回
/// - `Ok(Some(Message))`: 成功解码的应用层消息
/// - `Ok(None)`: 非文本消息（Ping/Pong/Close），可忽略
/// - `Err(NetworkError)`: 解析失败
pub fn decode_message(ws_msg: WsMessage) -> Result<Option<Message>, NetworkError> {
    match ws_msg {
        WsMessage::Text(text) => {
            // 解析 JSON
            let msg = Message::from_json(&text).map_err(|e| {
                NetworkError::MessageParseFailed(format!("JSON 解析失败: {}", e))
            })?;

            // 检查大小限制
            check_message_size(&msg)?;

            Ok(Some(msg))
        }
        WsMessage::Binary(_) => {
            Err(NetworkError::MessageParseFailed(
                "不支持二进制消息".to_string(),
            ))
        }
        WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Close(_) => {
            // WebSocket 协议消息，由 tokio-tungstenite 自动处理
            Ok(None)
        }
        WsMessage::Frame(_) => {
            // 原始帧，不应出现在应用层
            Err(NetworkError::MessageParseFailed(
                "收到未解析的原始帧".to_string(),
            ))
        }
    }
}

/// 计算重连延迟（指数退避）
///
/// # 参数
/// - `attempt`: 当前重连尝试次数（从 0 开始）
///
/// # 返回
/// - 延迟秒数（2^attempt * INITIAL_RECONNECT_DELAY_SECS）
///
/// # 示例
/// - attempt 0: 2 秒
/// - attempt 1: 4 秒
/// - attempt 2: 8 秒
pub fn calculate_reconnect_delay(attempt: u32) -> u64 {
    let multiplier = 2u64.pow(attempt);
    INITIAL_RECONNECT_DELAY_SECS * multiplier
}

// ============================================================
// 加密消息辅助函数
// ============================================================

/// 将应用层消息加密编码为 WebSocket 文本消息
///
/// 使用加密层的 session 对消息 payload 进行 AES-256-GCM 加密，
/// 然后封装为加密消息类型发送。
pub fn encode_encrypted_message(
    msg: &Message,
    crypto: &CryptoTransport,
    session: &CryptoSession,
) -> Result<WsMessage, NetworkError> {
    // 序列化原始消息
    let plaintext = msg
        .to_json()
        .map_err(|e| NetworkError::MessageParseFailed(format!("序列化失败: {}", e)))?;

    // 检查原始消息大小
    check_message_size(msg)?;

    // 加密
    let payload = crypto
        .encrypt(session, &plaintext)
        .map_err(|e| NetworkError::MessageParseFailed(format!("加密失败: {}", e)))?;

    // 封装为加密消息
    let enc_msg = Message::new_encrypted(payload.nonce, payload.ciphertext, payload.public_key);

    // 序列化为 JSON
    let json = enc_msg
        .to_json()
        .map_err(|e| NetworkError::MessageParseFailed(format!("序列化加密消息失败: {}", e)))?;

    Ok(WsMessage::Text(json))
}

/// 解密 WebSocket 消息
///
/// 如果消息是加密类型，解密后返回内部消息。
/// 如果消息是普通类型，直接返回。
/// 如果消息无法解码，返回错误。
pub fn decrypt_ws_message(
    ws_msg: WsMessage,
    crypto: &CryptoTransport,
    session: &mut CryptoSession,
    local_secret: &[u8; 32],
) -> Result<Option<Message>, NetworkError> {
    match ws_msg {
        WsMessage::Text(text) => {
            // 先解析为 Message
            let msg = Message::from_json(&text).map_err(|e| {
                NetworkError::MessageParseFailed(format!("JSON 解析失败: {}", e))
            })?;

            if msg.is_encrypted() {
                // 解密加密消息
                let (nonce, ciphertext, public_key) = msg.encrypted_payload().ok_or(
                    NetworkError::MessageParseFailed("无法提取加密载荷".to_string()),
                )?;

                let payload = EncryptedPayload {
                    nonce: nonce.to_vec(),
                    ciphertext: ciphertext.to_vec(),
                    public_key: public_key.to_vec(),
                };

                let plaintext = crypto
                    .decrypt(session, &payload, local_secret)
                    .map_err(|e| {
                        NetworkError::MessageParseFailed(format!("解密失败: {}", e))
                    })?;

                // 解析解密后的内部消息
                let inner_msg = Message::from_json(&plaintext).map_err(|e| {
                    NetworkError::MessageParseFailed(format!("解密内容解析失败: {}", e))
                })?;

                Ok(Some(inner_msg))
            } else {
                // 非加密消息，正常解析
                check_message_size(&msg)?;
                Ok(Some(msg))
            }
        }
        WsMessage::Binary(_) => Err(NetworkError::MessageParseFailed(
            "不支持二进制消息".to_string(),
        )),
        WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Close(_) => Ok(None),
        WsMessage::Frame(_) => Err(NetworkError::MessageParseFailed(
            "收到未解析的原始帧".to_string(),
        )),
    }
}

/// 检查心跳是否超时
///
/// # 参数
/// - `last_heartbeat_timestamp`: 上次收到心跳的 Unix 时间戳（秒）
///
/// # 返回
/// - `true`: 已超时
/// - `false`: 未超时
pub fn is_heartbeat_timeout(last_heartbeat_timestamp: i64) -> bool {
    let now = chrono::Utc::now().timestamp();
    let elapsed = now - last_heartbeat_timestamp;
    elapsed > HEARTBEAT_TIMEOUT_SECS as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_message_size_within_limit() {
        let msg = Message::new_clipboard(
            "Small content".to_string(),
            "device-1".to_string(),
        );

        assert!(check_message_size(&msg).is_ok());
    }

    #[test]
    fn test_check_message_size_exceeds_limit() {
        // 创建超过 10MB 的内容
        let large_content = "A".repeat(11 * 1024 * 1024);
        let msg = Message::new_clipboard(large_content, "device-1".to_string());

        let result = check_message_size(&msg);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NetworkError::ContentTooLarge { .. }));
    }

    #[test]
    fn test_encode_decode_message() {
        let original = Message::new_clipboard(
            "Test content".to_string(),
            "device-123".to_string(),
        );

        // 编码
        let ws_msg = encode_message(&original).unwrap();
        assert!(matches!(ws_msg, WsMessage::Text(_)));

        // 解码
        let decoded = decode_message(ws_msg).unwrap().unwrap();
        assert_eq!(decoded.content(), original.content());
        assert_eq!(decoded.device_id(), original.device_id());
    }

    #[test]
    fn test_decode_non_text_message() {
        // Ping 消息应返回 None
        let result = decode_message(WsMessage::Ping(vec![])).unwrap();
        assert!(result.is_none());

        // Pong 消息应返回 None
        let result = decode_message(WsMessage::Pong(vec![])).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_reconnect_delay() {
        assert_eq!(calculate_reconnect_delay(0), 2); // 2 秒
        assert_eq!(calculate_reconnect_delay(1), 4); // 4 秒
        assert_eq!(calculate_reconnect_delay(2), 8); // 8 秒
    }

    #[test]
    fn test_is_heartbeat_timeout() {
        let now = chrono::Utc::now().timestamp();

        // 30 秒前的心跳，未超时
        assert!(!is_heartbeat_timeout(now - 30));

        // 61 秒前的心跳，已超时
        assert!(is_heartbeat_timeout(now - 61));
    }
}

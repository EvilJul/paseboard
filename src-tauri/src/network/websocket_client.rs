// WebSocket 客户端
//
// 职责：
// - 连接到远程 WebSocket 服务端
// - 发送和接收消息
// - 自动重连（指数退避）
// - 心跳检测

use crate::utils::error::NetworkError;
use super::message::Message;
use super::websocket_common::{
    calculate_reconnect_delay, decode_message, encode_message, is_heartbeat_timeout,
    HEARTBEAT_INTERVAL_SECS, MAX_RECONNECT_ATTEMPTS,
};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// 客户端连接状态
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectionState {
    /// 未连接
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
}

/// WebSocket 客户端
pub struct WebSocketClient {
    /// 服务端地址（如 "ws://192.168.1.100:9527"）
    server_url: String,
    /// 设备 ID
    device_id: String,
    /// 连接状态
    state: Arc<RwLock<ConnectionState>>,
    /// 发送通道（应用层 -> WebSocket）
    send_tx: mpsc::UnboundedSender<Message>,
    /// 内部发送通道（用于实际 WebSocket 发送）
    internal_send_tx: Arc<RwLock<Option<mpsc::UnboundedSender<WsMessage>>>>,
    /// 消息接收通道（WebSocket -> 应用层）
    message_tx: mpsc::UnboundedSender<Message>,
    /// 上次收到心跳的时间戳
    last_heartbeat: Arc<RwLock<i64>>,
}

impl WebSocketClient {
    /// 创建 WebSocket 客户端
    ///
    /// # 参数
    /// - `server_url`: 服务端地址（如 "ws://192.168.1.100:9527"）
    /// - `device_id`: 本设备 ID
    ///
    /// # 返回
    /// - `(WebSocketClient, mpsc::UnboundedReceiver<Message>)`: 客户端实例和消息接收通道
    pub fn new(
        server_url: String,
        device_id: String,
    ) -> (Self, mpsc::UnboundedReceiver<Message>) {
        let (send_tx, send_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        let client = Self {
            server_url,
            device_id,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            send_tx,
            internal_send_tx: Arc::new(RwLock::new(None)),
            message_tx,
            last_heartbeat: Arc::new(RwLock::new(chrono::Utc::now().timestamp())),
        };

        // 启动发送任务
        client.spawn_send_task(send_rx);

        (client, message_rx)
    }

    /// 连接到服务端（带重连机制）
    pub async fn connect(&self) -> Result<(), NetworkError> {
        let mut attempt = 0;

        loop {
            // 更新状态为连接中
            {
                let mut state = self.state.write().await;
                *state = ConnectionState::Connecting;
            }

            info!("尝试连接到 {} (尝试 {}/{})", self.server_url, attempt + 1, MAX_RECONNECT_ATTEMPTS + 1);

            // 尝试连接
            match self.try_connect().await {
                Ok(ws_stream) => {
                    // 连接成功
                    {
                        let mut state = self.state.write().await;
                        *state = ConnectionState::Connected;
                    }

                    info!("成功连接到 {}", self.server_url);

                    // 重置心跳时间
                    {
                        let mut last_heartbeat = self.last_heartbeat.write().await;
                        *last_heartbeat = chrono::Utc::now().timestamp();
                    }

                    // 启动 WebSocket 读写任务
                    self.spawn_websocket_tasks(ws_stream).await;

                    // 启动心跳任务
                    self.spawn_heartbeat_task();

                    return Ok(());
                }
                Err(e) => {
                    warn!("连接失败: {}", e);

                    // 检查重连次数
                    if attempt >= MAX_RECONNECT_ATTEMPTS {
                        error!("达到最大重连次数 ({}), 放弃连接", MAX_RECONNECT_ATTEMPTS);
                        let mut state = self.state.write().await;
                        *state = ConnectionState::Disconnected;
                        return Err(e);
                    }

                    // 计算退避延迟
                    let delay = calculate_reconnect_delay(attempt);
                    info!("将在 {} 秒后重试", delay);
                    sleep(Duration::from_secs(delay)).await;

                    attempt += 1;
                }
            }
        }
    }

    /// 尝试连接到服务端（单次尝试）
    async fn try_connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, NetworkError> {
        let (ws_stream, _) = connect_async(&self.server_url)
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("连接失败: {}", e)))?;

        Ok(ws_stream)
    }

    /// 启动 WebSocket 读写任务
    async fn spawn_websocket_tasks(&self, ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>) {
        let message_tx = self.message_tx.clone();
        let state = Arc::clone(&self.state);
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let device_id = self.device_id.clone();
        let internal_send_tx = Arc::clone(&self.internal_send_tx);

        tokio::spawn(async move {
            let (mut ws_sink, mut ws_stream) = ws_stream.split();

            // 创建内部发送通道
            let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

            // 设置内部发送通道
            {
                let mut internal_tx = internal_send_tx.write().await;
                *internal_tx = Some(tx);
            }

            // 启动写任务
            let write_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if let Err(e) = ws_sink.send(msg).await {
                        error!("发送消息失败: {}", e);
                        break;
                    }
                }
            });

            // 启动读任务
            let internal_send_tx_for_read = Arc::clone(&internal_send_tx);
            let read_task = tokio::spawn(async move {
                while let Some(result) = ws_stream.next().await {
                    match result {
                        Ok(ws_msg) => {
                            // 解码消息
                            match decode_message(ws_msg) {
                                Ok(Some(msg)) => {
                                    // 更新心跳时间
                                    if msg.is_heartbeat() || msg.is_heartbeat_ack() {
                                        let mut last_hb = last_heartbeat.write().await;
                                        *last_hb = chrono::Utc::now().timestamp();

                                        // 收到心跳，发送响应
                                        if msg.is_heartbeat() {
                                            let ack = Message::new_heartbeat_ack(device_id.clone());
                                            if let Ok(ws_msg) = encode_message(&ack) {
                                                let internal_tx_read = internal_send_tx_for_read.read().await;
                                                if let Some(tx) = internal_tx_read.as_ref() {
                                                    let _ = tx.send(ws_msg);
                                                }
                                            }
                                        }
                                    } else {
                                        // 粘贴板消息转发给应用层
                                        if let Err(e) = message_tx.send(msg) {
                                            error!("转发消息到应用层失败: {}", e);
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => {
                                    // 协议消息，忽略
                                }
                                Err(e) => {
                                    warn!("消息解码失败: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("接收消息失败: {}", e);
                            break;
                        }
                    }
                }
            });

            // 等待任务结束
            let _ = tokio::join!(read_task, write_task);

            // 连接断开，清理内部发送通道
            {
                let mut internal_tx = internal_send_tx.write().await;
                *internal_tx = None;
            }

            // 更新状态
            {
                let mut state = state.write().await;
                *state = ConnectionState::Disconnected;
            }
            info!("WebSocket 连接已断开");
        });
    }

    /// 启动发送任务（从应用层通道接收消息并转发到 WebSocket）
    fn spawn_send_task(&self, mut send_rx: mpsc::UnboundedReceiver<Message>) {
        let state = Arc::clone(&self.state);
        let internal_send_tx = Arc::clone(&self.internal_send_tx);

        tokio::spawn(async move {
            while let Some(msg) = send_rx.recv().await {
                // 检查连接状态
                let current_state = state.read().await;
                if *current_state != ConnectionState::Connected {
                    warn!("连接未建立，消息发送失败");
                    continue;
                }
                drop(current_state);

                // 编码并发送
                match encode_message(&msg) {
                    Ok(ws_msg) => {
                        let internal_tx_read = internal_send_tx.read().await;
                        if let Some(tx) = internal_tx_read.as_ref() {
                            if let Err(e) = tx.send(ws_msg) {
                                error!("转发消息到 WebSocket 失败: {}", e);
                            } else {
                                debug!("消息已发送");
                            }
                        }
                    }
                    Err(e) => {
                        error!("消息编码失败: {}", e);
                    }
                }
            }
        });
    }

    /// 启动心跳任务
    fn spawn_heartbeat_task(&self) {
        let device_id = self.device_id.clone();
        let send_tx = self.send_tx.clone();
        let last_heartbeat = Arc::clone(&self.last_heartbeat);
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

            loop {
                ticker.tick().await;

                // 检查连接状态
                let current_state = state.read().await;
                if *current_state != ConnectionState::Connected {
                    break;
                }
                drop(current_state);

                // 检查心跳超时
                let last_hb = *last_heartbeat.read().await;
                if is_heartbeat_timeout(last_hb) {
                    error!("服务端心跳超时，连接已断开");
                    let mut state = state.write().await;
                    *state = ConnectionState::Disconnected;
                    break;
                }

                // 发送心跳
                let heartbeat = Message::new_heartbeat(device_id.clone());
                if let Err(e) = send_tx.send(heartbeat) {
                    error!("发送心跳失败: {}", e);
                    break;
                }
            }
        });
    }

    /// 发送消息
    pub async fn send(&self, msg: Message) -> Result<(), NetworkError> {
        // 检查连接状态
        let state = self.state.read().await;
        if *state != ConnectionState::Connected {
            return Err(NetworkError::ConnectionFailed("连接未建立".to_string()));
        }
        drop(state);

        // 发送到通道
        self.send_tx
            .send(msg)
            .map_err(|e| NetworkError::ConnectionFailed(format!("发送消息失败: {}", e)))?;

        Ok(())
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        let state = self.state.read().await;
        *state == ConnectionState::Connected
    }

    /// 获取服务端地址
    pub fn server_url(&self) -> &str {
        &self.server_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let (client, _rx) = WebSocketClient::new(
            "ws://127.0.0.1:9527".to_string(),
            "test-device".to_string(),
        );

        assert_eq!(client.server_url(), "ws://127.0.0.1:9527");
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_send_when_disconnected() {
        let (client, _rx) = WebSocketClient::new(
            "ws://127.0.0.1:9527".to_string(),
            "test-device".to_string(),
        );

        let msg = Message::new_clipboard("Test".to_string(), "device-1".to_string());
        let result = client.send(msg).await;

        assert!(result.is_err());
    }
}

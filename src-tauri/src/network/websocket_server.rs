// WebSocket 服务端
//
// 职责：
// - 监听指定端口，接受 WebSocket 连接
// - 管理已连接的客户端
// - 广播消息到所有客户端
// - 心跳检测和超时断开

use crate::utils::error::NetworkError;
use super::crypto::{CryptoSession, CryptoTransport};
use super::message::Message;
use super::websocket_common::{
    decrypt_ws_message, encode_encrypted_message, encode_message,
    is_heartbeat_timeout, HEARTBEAT_INTERVAL_SECS, HEARTBEAT_TIMEOUT_SECS,
};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

/// 客户端连接信息
struct ClientConnection {
    /// 发送通道（向客户端发送消息）
    tx: mpsc::UnboundedSender<WsMessage>,
    /// 上次收到心跳的时间戳
    last_heartbeat: i64,
    /// 客户端地址
    addr: SocketAddr,
    /// 设备 ID（首次消息解析后填充）
    device_id: String,
    /// 加密会话（首次加密消息时建立）
    session: Option<CryptoSession>,
}

/// WebSocket 服务端
pub struct WebSocketServer {
    /// 本地监听地址
    bind_addr: String,
    /// 设备 ID
    device_id: String,
    /// 已连接的客户端（key: 客户端地址）
    clients: Arc<RwLock<HashMap<SocketAddr, ClientConnection>>>,
    /// 消息接收通道（发送给应用层）
    message_tx: mpsc::UnboundedSender<Message>,
    /// 已连接的远程设备 ID 集合（通过服务端接入的）
    connected_device_ids: Arc<RwLock<HashSet<String>>>,
    /// 加密传输层
    crypto: Arc<CryptoTransport>,
}

impl WebSocketServer {
    /// 创建 WebSocket 服务端（同步方法，**立即绑定端口**，失败时返回错误）
    ///
    /// # 参数
    /// - `bind_addr`: 监听地址（如 "0.0.0.0:9527"）
    /// - `device_id`: 本设备 ID
    /// - `crypto`: 加密传输层
    ///
    /// # 返回
    /// - `Ok((WebSocketServer, mpsc::UnboundedReceiver<Message>, std::net::TcpListener))`:
    ///   服务端实例、消息接收通道、预绑定的 TCP listener
    /// - `Err(NetworkError)`: 端口绑定失败
    pub fn new(
        bind_addr: String,
        device_id: String,
        connected_device_ids: Arc<RwLock<HashSet<String>>>,
        crypto: Arc<CryptoTransport>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Message>, std::net::TcpListener), NetworkError> {
        // 同步预绑定端口，确保 mDNS 注册时端口一定可用
        let pre_bound_listener = std::net::TcpListener::bind(&bind_addr).map_err(|e| {
            NetworkError::ConnectionFailed(format!(
                "WebSocket 预绑定 {} 失败: {}",
                bind_addr, e
            ))
        })?;

        let (message_tx, message_rx) = mpsc::unbounded_channel();

        let server = Self {
            bind_addr,
            device_id,
            clients: Arc::new(RwLock::new(HashMap::new())),
            message_tx,
            connected_device_ids,
            crypto,
        };

        Ok((server, message_rx, pre_bound_listener))
    }

    /// 启动服务端（阻塞当前任务）
    pub async fn run(&self, listener: std::net::TcpListener) -> Result<(), NetworkError> {
        // 设置为非阻塞模式（Tokio 要求）
        listener.set_nonblocking(true).map_err(|e| {
            NetworkError::ConnectionFailed(format!("设置非阻塞模式失败: {}", e))
        })?;

        // 转换为 Tokio 异步 listener（listener 所有权转入此方法，保持端口持续监听）
        let listener = TcpListener::from_std(listener).map_err(|e| {
            NetworkError::ConnectionFailed(format!("转换异步监听器失败: {}", e))
        })?;

        info!("WebSocket 服务端启动，监听 {}", self.bind_addr);

        // 启动心跳检测任务
        self.spawn_heartbeat_task();

        // 接受连接循环
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("收到连接请求: {}", addr);
                    self.handle_connection(stream, addr).await;
                }
                Err(e) => {
                    error!("接受连接失败: {}", e);
                }
            }
        }
    }

    /// 处理单个客户端连接
    async fn handle_connection(&self, stream: TcpStream, addr: SocketAddr) {
        // WebSocket 握手
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket 握手失败 ({}): {}", addr, e);
                return;
            }
        };

        info!("WebSocket 连接已建立: {}", addr);

        // 分离读写流
        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        // 创建发送通道
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

        // 注册客户端
        {
            let mut clients = self.clients.write().await;
            clients.insert(
                addr,
                ClientConnection {
                    tx,
                    last_heartbeat: chrono::Utc::now().timestamp(),
                    addr,
                    device_id: String::new(),
                    session: None,
                },
            );
        }

        let clients = Arc::clone(&self.clients);
        let message_tx = self.message_tx.clone();
        let device_id = self.device_id.clone();
        let connected_device_ids = Arc::clone(&self.connected_device_ids);
        let crypto = Arc::clone(&self.crypto);
        let local_secret = self.crypto.local_secret_bytes();

        // 启动发送任务
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_sink.send(msg).await {
                    error!("发送消息失败 ({}): {}", addr, e);
                    break;
                }
            }
        });

        // 启动接收任务
        let recv_task = tokio::spawn(async move {
            while let Some(result) = ws_stream.next().await {
                match result {
                    Ok(ws_msg) => {
                        // 获取或创建客户端的加密 session
                        let decrypt_result = {
                            let mut clients_lock = clients.write().await;
                            let client = clients_lock.get_mut(&addr);

                            if let Some(client) = client {
                                if let Some(ref mut session) = client.session {
                                    decrypt_ws_message(ws_msg, &crypto, session, &local_secret)
                                } else {
                                    let mut temp_session = CryptoSession {
                                        key: [0u8; 32],
                                        remote_pubkey: [0u8; 32],
                                    };
                                    let result = decrypt_ws_message(
                                        ws_msg,
                                        &crypto,
                                        &mut temp_session,
                                        &local_secret,
                                    );
                                    if result.is_ok() && temp_session.key != [0u8; 32] {
                                        client.session = Some(temp_session);
                                    }
                                    result
                                }
                            } else {
                                Ok(None)
                            }
                        };

                        match decrypt_result {
                            Ok(Some(msg)) => {
                                // 更新心跳时间
                                if msg.is_heartbeat() || msg.is_heartbeat_ack() {
                                    let mut clients_lock = clients.write().await;
                                    if let Some(client) = clients_lock.get_mut(&addr) {
                                        client.last_heartbeat = chrono::Utc::now().timestamp();
                                        if client.device_id.is_empty() {
                                            let new_id = msg.device_id().to_string();
                                            client.device_id = new_id.clone();
                                            drop(clients_lock);
                                            connected_device_ids.write().await.insert(new_id);
                                        } else {
                                            drop(clients_lock);
                                        }
                                    } else {
                                        drop(clients_lock);
                                    }

                                    // 心跳消息需要响应
                                    if msg.is_heartbeat() {
                                        let ack = Message::new_heartbeat_ack(device_id.clone());
                                        let clients_read = clients.read().await;
                                        if let Some(client) = clients_read.get(&addr) {
                                            let ws_msg = if let Some(ref session) = client.session
                                            {
                                                encode_encrypted_message(
                                                    &ack,
                                                    &crypto,
                                                    session,
                                                )
                                                .unwrap_or_else(|_| {
                                                    encode_message(&ack).unwrap_or_else(|e| {
                                                        error!("心跳响应编码失败: {}", e);
                                                        // 返回一个空的心跳消息作为 fallback
                                                        WsMessage::Text(
                                                            serde_json::json!({
                                                                "type": "heartbeat_ack",
                                                                "device_id": "",
                                                                "timestamp": 0
                                                            })
                                                            .to_string(),
                                                        )
                                                    })
                                                })
                                            } else {
                                                encode_message(&ack).unwrap_or_else(|e| {
                                                    error!("心跳响应编码失败: {}", e);
                                                    WsMessage::Text(
                                                        serde_json::json!({
                                                            "type": "heartbeat_ack",
                                                            "device_id": "",
                                                            "timestamp": 0
                                                        })
                                                        .to_string(),
                                                    )
                                                })
                                            };
                                            let _ = client.tx.send(ws_msg);
                                        }
                                    }
                                } else {
                                    // 粘贴板消息 - 首次消息注册设备 ID
                                    {
                                        let mut clients_lock = clients.write().await;
                                        if let Some(client) = clients_lock.get_mut(&addr) {
                                            if client.device_id.is_empty() {
                                                let new_id = msg.device_id().to_string();
                                                client.device_id = new_id.clone();
                                                drop(clients_lock);
                                                connected_device_ids.write().await.insert(new_id);
                                            }
                                        }
                                    }
                                    // 转发给应用层
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
                                warn!("消息解码失败 ({}): {}", addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("接收消息失败 ({}): {}", addr, e);
                        break;
                    }
                }
            }

            // 连接断开，移除客户端
            let mut clients = clients.write().await;
            if let Some(client) = clients.remove(&addr) {
                if !client.device_id.is_empty() {
                    connected_device_ids.write().await.remove(&client.device_id);
                }
            }
            info!("WebSocket 连接已断开: {}", addr);
        });

        // 等待任务结束
        let _ = tokio::join!(send_task, recv_task);
    }

    /// 广播消息到所有客户端
    ///
    /// 对有 session 的客户端加密发送，无 session 的客户端明文发送。
    pub async fn broadcast(&self, msg: &Message) -> Result<(), NetworkError> {
        let clients = self.clients.read().await;

        for (addr, client) in clients.iter() {
            let ws_msg = if let Some(ref session) = client.session {
                encode_encrypted_message(msg, &self.crypto, session)
                    .unwrap_or_else(|_| encode_message(msg).unwrap())
            } else {
                encode_message(msg).unwrap()
            };

            let tx = client.tx.clone();
            let addr = *addr;
            tokio::spawn(async move {
                if let Err(e) = tx.send(ws_msg) {
                    warn!("广播消息失败 ({}): {}", addr, e);
                }
            });
        }

        debug!("消息已广播到 {} 个客户端", clients.len());
        Ok(())
    }

    /// 启动心跳检测任务（每 30 秒检查一次）
    fn spawn_heartbeat_task(&self) {
        let clients = Arc::clone(&self.clients);
        let device_id = self.device_id.clone();
        let crypto = Arc::clone(&self.crypto);

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

            loop {
                ticker.tick().await;

                let mut clients = clients.write().await;
                let mut disconnected = Vec::new();

                // 检查超时客户端
                for (addr, client) in clients.iter() {
                    if is_heartbeat_timeout(client.last_heartbeat) {
                        warn!(
                            "客户端心跳超时 ({}): {} 秒未响应",
                            addr, HEARTBEAT_TIMEOUT_SECS
                        );
                        disconnected.push(*addr);
                    } else {
                        // 发送心跳（优先加密）
                        let heartbeat = Message::new_heartbeat(device_id.clone());
                        let ws_msg = if let Some(ref session) = client.session {
                            encode_encrypted_message(&heartbeat, &crypto, session)
                                .unwrap_or_else(|_| encode_message(&heartbeat).unwrap())
                        } else {
                            encode_message(&heartbeat).unwrap()
                        };
                        let _ = client.tx.send(ws_msg);
                    }
                }

                // 移除超时客户端
                for addr in disconnected {
                    clients.remove(&addr);
                    info!("已断开超时客户端: {}", addr);
                }
            }
        });
    }

    /// 获取当前连接的客户端数量
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// 获取已连接的远程设备 ID 集合引用
    pub fn get_connected_device_ids_ref(&self) -> Arc<RwLock<HashSet<String>>> {
        Arc::clone(&self.connected_device_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::crypto::CryptoTransport;
    use crate::network::identity::IdentityManager;

    fn create_test_crypto() -> Arc<CryptoTransport> {
        let identity_path = std::env::temp_dir().join("paseboard_test_ws_server_id.pem");
        let _ = std::fs::remove_file(&identity_path);
        let identity = Arc::new(IdentityManager::new(identity_path.clone()).unwrap());
        let crypto = Arc::new(CryptoTransport::new(identity));
        let _ = std::fs::remove_file(&identity_path);
        crypto
    }

    fn make_connected_ids() -> Arc<RwLock<HashSet<String>>> {
        Arc::new(RwLock::new(HashSet::new()))
    }

    #[tokio::test]
    async fn test_server_creation() {
        let crypto = create_test_crypto();
        let (server, _rx, _listener) = WebSocketServer::new(
            "127.0.0.1:9527".to_string(),
            "test-device".to_string(),
            make_connected_ids(),
            crypto,
        )
        .unwrap();

        assert_eq!(server.bind_addr, "127.0.0.1:9527");
        assert_eq!(server.device_id, "test-device");
        assert_eq!(server.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_no_clients() {
        let crypto = create_test_crypto();
        let (server, _rx, _listener) = WebSocketServer::new(
            "127.0.0.1:9528".to_string(),
            "test-device".to_string(),
            make_connected_ids(),
            crypto,
        )
        .unwrap();

        let msg = Message::new_clipboard("Test".to_string(), "device-1".to_string());
        let result = server.broadcast(&msg).await;

        assert!(result.is_ok());
    }
}

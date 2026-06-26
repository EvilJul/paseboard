// WebSocket 服务端
//
// 职责：
// - 监听指定端口，接受 WebSocket 连接
// - 管理已连接的客户端
// - 广播消息到所有客户端
// - 心跳检测和超时断开

use crate::utils::error::NetworkError;
use super::message::Message;
use super::websocket_common::{
    decode_message, encode_message, is_heartbeat_timeout, HEARTBEAT_INTERVAL_SECS,
    HEARTBEAT_TIMEOUT_SECS,
};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::collections::HashMap;
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
}

impl WebSocketServer {
    /// 创建 WebSocket 服务端（同步方法，**立即绑定端口**，失败时返回错误）
    ///
    /// # 参数
    /// - `bind_addr`: 监听地址（如 "0.0.0.0:9527"）
    /// - `device_id`: 本设备 ID
    ///
    /// # 返回
    /// - `Ok((WebSocketServer, mpsc::UnboundedReceiver<Message>))`: 服务端实例和消息接收通道
    /// - `Err(NetworkError)`: 端口绑定失败
    pub fn new(
        bind_addr: String,
        device_id: String,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Message>), NetworkError> {
        // 同步预绑定端口，确保 mDNS 注册时端口一定可用
        std::net::TcpListener::bind(&bind_addr).map_err(|e| {
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
        };

        Ok((server, message_rx))
    }

    /// 启动服务端（阻塞当前任务）
    pub async fn run(&self) -> Result<(), NetworkError> {
        // 绑定监听地址（端口在 new() 中已预绑定，这里再次绑定成功即可）
        let listener = TcpListener::bind(&self.bind_addr)
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("绑定失败: {}", e)))?;

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
                },
            );
        }

        let clients = Arc::clone(&self.clients);
        let message_tx = self.message_tx.clone();
        let device_id = self.device_id.clone();

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
                        // 解码消息
                        match decode_message(ws_msg) {
                            Ok(Some(msg)) => {
                                // 更新心跳时间
                                if msg.is_heartbeat() || msg.is_heartbeat_ack() {
                                    let mut clients_lock = clients.write().await;
                                    if let Some(client) = clients_lock.get_mut(&addr) {
                                        client.last_heartbeat = chrono::Utc::now().timestamp();
                                    }
                                    drop(clients_lock);

                                    // 心跳消息需要响应
                                    if msg.is_heartbeat() {
                                        let ack = Message::new_heartbeat_ack(device_id.clone());
                                        if let Ok(ws_msg) = encode_message(&ack) {
                                            let clients_read = clients.read().await;
                                            if let Some(client) = clients_read.get(&addr) {
                                                let _ = client.tx.send(ws_msg);
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
            clients.remove(&addr);
            info!("WebSocket 连接已断开: {}", addr);
        });

        // 等待任务结束
        let _ = tokio::join!(send_task, recv_task);
    }

    /// 广播消息到所有客户端（优化：序列化一次 + 并发发送）
    pub async fn broadcast(&self, msg: &Message) -> Result<(), NetworkError> {
        // 序列化一次
        let ws_msg = encode_message(msg)?;

        // 并发发送到所有客户端
        let clients = self.clients.read().await;
        let mut send_tasks = Vec::new();

        for (addr, client) in clients.iter() {
            let tx = client.tx.clone();
            let ws_msg = ws_msg.clone();
            let addr = *addr;

            let task = tokio::spawn(async move {
                if let Err(e) = tx.send(ws_msg) {
                    warn!("广播消息失败 ({}): {}", addr, e);
                }
            });

            send_tasks.push(task);
        }

        // 等待所有发送任务完成
        for task in send_tasks {
            let _ = task.await;
        }

        debug!("消息已广播到 {} 个客户端", clients.len());
        Ok(())
    }

    /// 启动心跳检测任务（每 30 秒检查一次）
    fn spawn_heartbeat_task(&self) {
        let clients = Arc::clone(&self.clients);
        let device_id = self.device_id.clone();

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
                        // 发送心跳
                        let heartbeat = Message::new_heartbeat(device_id.clone());
                        if let Ok(ws_msg) = encode_message(&heartbeat) {
                            let _ = client.tx.send(ws_msg);
                        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let (server, _rx) = WebSocketServer::new(
            "127.0.0.1:9527".to_string(),
            "test-device".to_string(),
        )
        .unwrap();

        assert_eq!(server.bind_addr, "127.0.0.1:9527");
        assert_eq!(server.device_id, "test-device");
        assert_eq!(server.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_no_clients() {
        let (server, _rx) = WebSocketServer::new(
            "127.0.0.1:9528".to_string(),
            "test-device".to_string(),
        )
        .unwrap();

        let msg = Message::new_clipboard("Test".to_string(), "device-1".to_string());
        let result = server.broadcast(&msg).await;

        assert!(result.is_ok());
    }
}

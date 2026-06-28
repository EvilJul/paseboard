// 应用主逻辑协调模块
//
// 职责：
// - 初始化所有模块（mDNS、WebSocket、Storage、Monitor、Writer、Dedup）
// - 协调设备发现到连接建立的流程
// - 协调粘贴板监听到消息推送的流程
// - 协调消息接收到粘贴板写入的流程
// - 并行启动优化（mDNS、WebSocket、Storage 并行初始化）

use crate::config::AppConfig;
use crate::network::mdns::{MdnsService, DeviceInfo};
use crate::network::websocket_server::WebSocketServer;
use crate::network::websocket_client::WebSocketClient;
use crate::network::message::Message;
use crate::clipboard::monitor::{ClipboardMonitor, ClipboardChange};
use crate::clipboard::writer::ClipboardWriter;
use crate::clipboard::storage::{HistoryStorage, HistoryItem};
use crate::clipboard::dedup::DeduplicationService;

use log::{info, warn, error, debug};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use sha2::{Sha256, Digest};

/// 设备信息快照（用于 IPC 返回，字段名与前端期望一致）
#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub id: String,
    pub name: String,
    pub addr: String,
    pub port: u16,
    pub last_seen: u64,
}

impl DeviceSnapshot {
    pub fn is_offline(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // 与 mdns.rs 保持一致的 30 秒超时
        now.saturating_sub(self.last_seen) > 30
    }
}

impl From<DeviceInfo> for DeviceSnapshot {
    fn from(d: DeviceInfo) -> Self {
        Self {
            id: d.id,
            name: d.name,
            addr: d.addr.to_string(),
            port: d.port,
            last_seen: d.last_seen,
        }
    }
}

/// 历史记录存储请求
#[derive(Debug, Clone)]
struct StorageRequest {
    content: String,
    device_id: String,
    device_name: String,
}

/// 历史记录查询请求（带 oneshot 回复通道）
pub struct StorageQuery {
    pub limit: usize,
    pub reply: tokio::sync::oneshot::Sender<anyhow::Result<Vec<crate::clipboard::storage::HistoryItem>>>,
}

/// 应用主协调器
pub struct App {
    /// 应用配置
    config: AppConfig,
    /// mDNS 服务
    mdns: Arc<MdnsService>,
    /// WebSocket 服务端
    ws_server: Arc<WebSocketServer>,
    /// WebSocket 服务端消息接收通道
    ws_server_rx: Arc<RwLock<mpsc::UnboundedReceiver<Message>>>,
    /// 粘贴板监听器
    clipboard_monitor: ClipboardMonitor,
    /// 粘贴板监听器事件接收通道
    clipboard_rx: Arc<RwLock<mpsc::UnboundedReceiver<ClipboardChange>>>,
    /// 粘贴板写入器
    clipboard_writer: Arc<ClipboardWriter>,
    /// 历史存储请求发送通道
    storage_tx: mpsc::UnboundedSender<StorageRequest>,
    /// 历史存储查询发送通道
    storage_query_tx: mpsc::UnboundedSender<StorageQuery>,
    /// 去重服务
    dedup_service: Arc<DeduplicationService>,
    /// 已连接的客户端（设备 ID -> WebSocketClient）
    clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
    /// 已连接的入站远程设备 ID 集合
    server_connected_device_ids: Arc<RwLock<HashSet<String>>>,
}

/// 应用对外暴露给 IPC 命令使用的句柄（轻量、Send + Sync）
pub struct IpcHandles {
    /// mDNS 服务（用于查询发现的设备列表）
    pub mdns: Arc<MdnsService>,
    /// 历史存储查询通道
    pub storage_query_tx: mpsc::UnboundedSender<StorageQuery>,
    /// 已连接的出站 WebSocket 客户端
    pub clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
    /// 已连接的入站远程设备 ID 集合
    pub server_connected_device_ids: Arc<RwLock<HashSet<String>>>,
}

impl App {
    /// 创建应用实例（并行初始化）
    ///
    /// # 参数
    /// - `config`: 应用配置
    /// - `app_handle`: Tauri 应用句柄
    ///
    /// # 返回
    /// 初始化后的应用实例
    pub async fn new(config: AppConfig, app_handle: tauri::AppHandle) -> anyhow::Result<Self> {
        info!("开始初始化应用...");

        // 创建共享的入站连接设备 ID 集合
        let server_connected_device_ids: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

        // 并行初始化三个独立模块：mDNS、WebSocket Server、Storage
        let mdns_handle = {
            let device_id = config.device_id.clone();
            let device_name = config.device_name.clone();
            let port = config.port;
            tokio::spawn(async move {
                MdnsService::new(device_id, device_name, port)
            })
        };

        let ws_server_handle = {
            let bind_addr = format!("0.0.0.0:{}", config.port);
            let device_id = config.device_id.clone();
            let connected_ids = Arc::clone(&server_connected_device_ids);
            tokio::spawn(async move {
                WebSocketServer::new(bind_addr, device_id, connected_ids)
            })
        };

        let storage_handle = {
            let db_path = config.db_path()?;
            tokio::spawn(async move {
                HistoryStorage::new(db_path)
            })
        };

        // 等待并行初始化完成
        let mdns = mdns_handle.await??;
        let (ws_server, ws_server_rx) = ws_server_handle.await??;
        let history_storage = storage_handle.await??;

        info!("mDNS、WebSocket Server、Storage 并行初始化完成");

        // 注册 mDNS 服务
        mdns.register()?;
        info!("mDNS 服务已注册，端口: {}", mdns.get_port());

        // 初始化粘贴板监听器和写入器
        let (clipboard_monitor, clipboard_rx) = ClipboardMonitor::new(app_handle.clone());
        let clipboard_writer = ClipboardWriter::new(app_handle);

        // 创建去重服务
        let dedup_service = DeduplicationService::new();

        // 创建存储请求通道
        let (storage_tx, storage_rx) = mpsc::unbounded_channel();
        // 创建存储查询通道
        let (storage_query_tx, storage_query_rx) = mpsc::unbounded_channel::<StorageQuery>();

        // 启动存储处理任务（在独立线程中，因为 rusqlite 不是 Send）
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("创建存储任务运行时失败");
            rt.block_on(async move {
                Self::handle_storage_requests(history_storage, storage_rx, storage_query_rx).await;
            });
        });

        info!("应用初始化完成");

        Ok(Self {
            config,
            mdns: Arc::new(mdns),
            ws_server: Arc::new(ws_server),
            ws_server_rx: Arc::new(RwLock::new(ws_server_rx)),
            clipboard_monitor,
            clipboard_rx: Arc::new(RwLock::new(clipboard_rx)),
            clipboard_writer: Arc::new(clipboard_writer),
            storage_tx,
            storage_query_tx,
            dedup_service: Arc::new(dedup_service),
            clients: Arc::new(RwLock::new(HashMap::new())),
            server_connected_device_ids,
        })
    }

    /// 处理存储请求和查询（在独立线程中运行）
    async fn handle_storage_requests(
        mut storage: HistoryStorage,
        mut storage_rx: mpsc::UnboundedReceiver<StorageRequest>,
        mut storage_query_rx: mpsc::UnboundedReceiver<StorageQuery>,
    ) {
        info!("存储处理任务已启动");

        loop {
            tokio::select! {
                Some(req) = storage_rx.recv() => {
                    if let Err(e) = storage.insert(&req.content, &req.device_id, &req.device_name) {
                        error!("保存历史记录失败: {}", e);
                    }
                }
                Some(query) = storage_query_rx.recv() => {
                    let result = storage
                        .query_recent(query.limit)
                        .map_err(|e| anyhow::anyhow!("查询历史记录失败: {}", e));
                    let _ = query.reply.send(result);
                }
                else => break,
            }
        }
    }

    /// 运行应用主循环
    pub async fn run(self) -> anyhow::Result<()> {
        info!("应用主循环启动");

        // 提取需要移动的字段
        let Self {
            config,
            mdns,
            ws_server,
            ws_server_rx,
            clipboard_monitor,
            clipboard_rx,
            clipboard_writer,
            storage_tx,
            storage_query_tx: _, // 已在 Self 中持有，无须再传递
            dedup_service,
            clients,
            server_connected_device_ids,
        } = self;

        // 启动 WebSocket 服务端（独立任务）
        let ws_server_for_run = Arc::clone(&ws_server);
        tokio::spawn(async move {
            if let Err(e) = ws_server_for_run.run().await {
                error!("WebSocket 服务端运行失败: {}", e);
            }
        });

        // 启动 mDNS 监听（使用 spawn_blocking 因为 listen() 是阻塞调用）
        let mdns_for_listen = Arc::clone(&mdns);
        std::thread::spawn(move || {
            if let Err(e) = mdns_for_listen.listen() {
                error!("mDNS 监听失败: {}", e);
            }
        });

        // 启动 UDP 广播发现（作为 mDNS 的备用方案，避免 macOS 端口 5353 冲突）
        mdns.start_broadcast_discovery();

        // 启动粘贴板监听器（独立任务）
        tokio::spawn(async move {
            clipboard_monitor.start().await;
        });

        // 启动设备发现到连接建立的流程（独立任务）
        {
            let mdns_for_discovery = Arc::clone(&mdns);
            let config_for_discovery = config.clone();
            let clients_for_discovery = Arc::clone(&clients);
            let server_ids_for_discovery = Arc::clone(&server_connected_device_ids);
            let dedup_service_for_discovery = Arc::clone(&dedup_service);
            let clipboard_writer_for_discovery = Arc::clone(&clipboard_writer);
            let storage_tx_for_discovery = storage_tx.clone();

            tokio::spawn(async move {
                Self::handle_device_discovery_task(
                    mdns_for_discovery,
                    config_for_discovery,
                    clients_for_discovery,
                    server_ids_for_discovery,
                    dedup_service_for_discovery,
                    clipboard_writer_for_discovery,
                    storage_tx_for_discovery,
                ).await;
            });
        }

        // 启动粘贴板变化到消息推送的流程（独立任务）
        {
            let clipboard_rx_for_changes = Arc::clone(&clipboard_rx);
            let dedup_service_for_changes = Arc::clone(&dedup_service);
            let ws_server_for_changes = Arc::clone(&ws_server);
            let clients_for_changes = Arc::clone(&clients);
            let storage_tx_for_changes = storage_tx.clone();
            let config_for_changes = config.clone();

            tokio::spawn(async move {
                Self::handle_clipboard_changes_task(
                    clipboard_rx_for_changes,
                    dedup_service_for_changes,
                    ws_server_for_changes,
                    clients_for_changes,
                    storage_tx_for_changes,
                    config_for_changes,
                ).await;
            });
        }

        // 主任务：处理 WebSocket 服务端接收到的消息
        Self::handle_incoming_messages_task(
            ws_server_rx,
            dedup_service,
            clipboard_writer,
            storage_tx,
        ).await;

        Ok(())
    }

    /// 处理设备发现到连接建立的流程（静态方法）
    ///
    /// 流程：mDNS 发现设备 → 获取设备信息 → WebSocketClient 连接
    async fn handle_device_discovery_task(
        mdns: Arc<MdnsService>,
        config: AppConfig,
        clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
        server_connected_ids: Arc<RwLock<HashSet<String>>>,
        dedup_service: Arc<DeduplicationService>,
        clipboard_writer: Arc<ClipboardWriter>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
    ) {
        info!("设备发现流程已启动");

        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));

        loop {
            ticker.tick().await;

            // 同步已连接的设备到 mDNS 设备列表，防止被清理任务删除
            {
                let clients_read = clients.read().await;
                for id in clients_read.keys() {
                    mdns.update_device_heartbeat(id);
                }
            }
            {
                let server_read = server_connected_ids.read().await;
                for id in server_read.iter() {
                    mdns.update_device_heartbeat(id);
                }
            }

            // 获取当前发现的设备列表
            let devices = mdns.get_devices();
            debug!("设备发现周期: 共发现 {} 台设备", devices.len());

            for device in devices {
                // 跳过已连接的设备
                {
                    let clients_read = clients.read().await;
                    if clients_read.contains_key(&device.id) {
                        continue;
                    }
                }

                // 尝试连接到新发现的设备
                info!("发现新设备: {} ({}:{})", device.name, device.addr, device.port);
                Self::connect_to_device(
                    device,
                    config.clone(),
                    Arc::clone(&clients),
                    Arc::clone(&dedup_service),
                    Arc::clone(&clipboard_writer),
                    storage_tx.clone(),
                ).await;
            }
        }
    }

    /// 连接到指定设备（静态方法）
    /// 
    /// 使用 device_id 比较避免双向连接冲突：
    /// - 只有 own_device_id < remote_device_id 时才主动连接
    /// - 否则等待对方连接（通过 WebSocket 服务端接受）
    async fn connect_to_device(
        device: DeviceInfo,
        config: AppConfig,
        clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
        dedup_service: Arc<DeduplicationService>,
        clipboard_writer: Arc<ClipboardWriter>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
    ) {
        // 避免双向连接冲突：只有本设备 ID 较小时才主动连接
        info!("设备 ID 比较: 本设备={}, 远程设备={}, 本设备<远程={}", 
              config.device_id, device.id, config.device_id < device.id);
        
        if config.device_id >= device.id {
            info!("跳过连接 {}：等待对方连接（本设备 ID 较大）", device.name);
            return;
        }
        
        let server_url = format!("ws://{}:{}", device.addr, device.port);
        let device_id = config.device_id.clone();

        info!("尝试连接到设备 {}: {}", device.name, server_url);

        // 创建 WebSocket 客户端
        let (client, mut client_rx) = WebSocketClient::new(server_url.clone(), device_id.clone());
        let client = Arc::new(client);

        // 注册客户端
        {
            let mut clients_write = clients.write().await;
            clients_write.insert(device.id.clone(), Arc::clone(&client));
        }

        // 启动连接任务
        let client_for_connect = Arc::clone(&client);
        let device_name = device.name.clone();
        let device_id_for_connect = device.id.clone();
        let clients_for_cleanup = Arc::clone(&clients);

        tokio::spawn(async move {
            match client_for_connect.connect().await {
                Ok(_) => {
                    info!("成功连接到设备: {}", device_name);

                    // 等待断开通知，然后清理客户端状态
                    let mut disc_rx = client_for_connect.disconnect_receiver();
                    let _ = disc_rx.changed().await;

                    // 连接已断开，从 clients 中移除
                    let mut clients = clients_for_cleanup.write().await;
                    clients.remove(&device_id_for_connect);
                    info!("设备 {} 断开连接，已清理客户端状态（将自动重连）", device_name);
                }
                Err(e) => {
                    error!("连接设备 {} 失败: {}", device_name, e);
                    // 连接失败，清理客户端
                    let mut clients = clients_for_cleanup.write().await;
                    clients.remove(&device_id_for_connect);
                }
            }
        });

        // 启动消息接收任务
        let device_name = device.name.clone();
        let device_id_for_rx = device.id.clone();

        tokio::spawn(async move {
            while let Some(msg) = client_rx.recv().await {
                if msg.is_clipboard() {
                    if let (Some(content), Some(uuid)) = (msg.content(), msg.uuid()) {
                        // 计算内容哈希
                        let content_hash = Self::calculate_hash(content);

                        // 去重检查
                        if !dedup_service.should_process_message(uuid, &content_hash).await {
                            continue;
                        }

                        // 写入粘贴板
                        match clipboard_writer.write(content.to_string(), uuid.to_string()).await {
                            Ok(true) => {
                                info!("收到来自 {} 的粘贴板内容，已写入本地", device_name);

                                // 标记消息已处理
                                dedup_service.mark_message_processed(uuid.to_string(), content_hash).await;

                                // 发送存储请求
                                let _ = storage_tx.send(StorageRequest {
                                    content: content.to_string(),
                                    device_id: device_id_for_rx.clone(),
                                    device_name: device_name.clone(),
                                });
                            }
                            Ok(false) => {
                                debug!("消息 UUID {} 已处理过，跳过写入", uuid);
                            }
                            Err(e) => {
                                error!("写入粘贴板失败: {}", e);
                            }
                        }
                    }
                }
            }
        });
    }

    /// 处理粘贴板变化到消息推送的流程（静态方法）
    ///
    /// 流程：ClipboardMonitor 检测变化 → DeduplicationService 检查 →
    ///       WebSocketServer 广播 → HistoryStorage 记录
    async fn handle_clipboard_changes_task(
        clipboard_rx: Arc<RwLock<mpsc::UnboundedReceiver<ClipboardChange>>>,
        dedup_service: Arc<DeduplicationService>,
        ws_server: Arc<WebSocketServer>,
        clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
        config: AppConfig,
    ) {
        info!("粘贴板变化处理流程已启动");

        let mut clipboard_rx = clipboard_rx.write().await;

        while let Some(change) = clipboard_rx.recv().await {
            debug!("检测到粘贴板变化，内容哈希: {}", change.hash);

            // 去重检查（发送端）
            if !dedup_service.should_send_message(&change.hash).await {
                debug!("内容哈希 {} 重复，跳过推送", change.hash);
                continue;
            }

            // 创建粘贴板消息
            let msg = Message::new_clipboard(
                change.content.clone(),
                config.device_id.clone(),
            );

            // 广播到 WebSocket 服务端的所有客户端
            if let Err(e) = ws_server.broadcast(&msg).await {
                error!("广播消息到服务端客户端失败: {}", e);
            } else {
                info!("广播粘贴板内容到服务端客户端，内容长度: {} 字节", change.content.len());
            }

            // 发送到所有 WebSocket 客户端（连接到其他设备）
            let clients_read = clients.read().await;
            for (device_id, client) in clients_read.iter() {
                if let Err(e) = client.send(msg.clone()).await {
                    warn!("发送消息到设备 {} 失败: {}", device_id, e);
                }
            }

            // 标记消息已发送
            dedup_service.mark_message_sent(change.hash).await;

            // 发送存储请求
            let _ = storage_tx.send(StorageRequest {
                content: change.content,
                device_id: config.device_id.clone(),
                device_name: config.device_name.clone(),
            });
        }
    }

    /// 处理 WebSocket 服务端接收到的消息（静态方法）
    ///
    /// 流程：WebSocketServer 接收消息 → DeduplicationService 检查 →
    ///       ClipboardWriter 写入 → HistoryStorage 记录
    async fn handle_incoming_messages_task(
        ws_server_rx: Arc<RwLock<mpsc::UnboundedReceiver<Message>>>,
        dedup_service: Arc<DeduplicationService>,
        clipboard_writer: Arc<ClipboardWriter>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
    ) {
        info!("消息接收处理流程已启动");

        let mut ws_server_rx = ws_server_rx.write().await;

        while let Some(msg) = ws_server_rx.recv().await {
            if msg.is_clipboard() {
                if let (Some(content), Some(uuid)) = (msg.content(), msg.uuid()) {
                    // 计算内容哈希
                    let content_hash = Self::calculate_hash(content);

                    // 去重检查
                    if !dedup_service.should_process_message(uuid, &content_hash).await {
                        continue;
                    }

                    // 写入粘贴板
                    match clipboard_writer.write(content.to_string(), uuid.to_string()).await {
                        Ok(true) => {
                            info!("收到粘贴板内容，已写入本地，内容长度: {} 字节", content.len());

                            // 标记消息已处理
                            dedup_service.mark_message_processed(uuid.to_string(), content_hash).await;

                            // 发送存储请求
                            let device_id = msg.device_id();
                            let _ = storage_tx.send(StorageRequest {
                                content: content.to_string(),
                                device_id: device_id.to_string(),
                                device_name: "Unknown Device".to_string(), // 服务端接收的消息可能没有设备名称
                            });
                        }
                        Ok(false) => {
                            debug!("消息 UUID {} 已处理过，跳过写入", uuid);
                        }
                        Err(e) => {
                            error!("写入粘贴板失败: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// 计算内容哈希（SHA256）
    fn calculate_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 构造 IPC 命令需要的句柄（在 run() 之前调用，提取共享引用）
    pub fn ipc_handles(&self) -> IpcHandles {
        IpcHandles {
            mdns: Arc::clone(&self.mdns),
            storage_query_tx: self.storage_query_tx.clone(),
            clients: Arc::clone(&self.clients),
            server_connected_device_ids: Arc::clone(&self.server_connected_device_ids),
        }
    }

    /// 获取当前已发现设备列表快照（供 IPC 使用）
    pub async fn get_devices_snapshot(&self) -> Vec<DeviceSnapshot> {
        self.mdns
            .get_devices()
            .into_iter()
            .map(DeviceSnapshot::from)
            .collect()
    }

    /// 获取最近的历史记录（供 IPC 使用，跨线程走 storage_query_tx 通道）
    pub async fn get_recent_history(&self, limit: usize) -> anyhow::Result<Vec<HistoryItem>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.storage_query_tx
            .send(StorageQuery { limit, reply: tx })
            .map_err(|_| anyhow::anyhow!("存储查询通道已关闭"))?;
        rx.await
            .map_err(|_| anyhow::anyhow!("存储查询响应失败"))?
    }
}

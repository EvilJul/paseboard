// 应用主逻辑协调模块
//
// 职责：
// - 初始化所有模块（mDNS、WebSocket、Storage、Monitor、Writer、Dedup）
// - 协调设备发现到连接建立的流程
// - 协调粘贴板监听到消息推送的流程
// - 协调消息接收到粘贴板写入的流程
// - 并行启动优化（mDNS、WebSocket、Storage 并行初始化）

use crate::config::AppConfig;
use crate::network::crypto::CryptoTransport;
use crate::network::identity::IdentityManager;
use crate::network::mdns::{MdnsService, DeviceInfo, CRYPTO_VERSION};
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
use tauri::Emitter;
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
    pub is_compatible: bool,
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
        let is_compatible = d.is_compatible();
        Self {
            id: d.id,
            name: d.name,
            addr: d.addr.to_string(),
            port: d.port,
            last_seen: d.last_seen,
            is_compatible,
        }
    }
}

/// 历史记录存储请求
#[derive(Debug, Clone)]
struct StorageRequest {
    content: String,
    content_type: String,
    device_id: String,
    device_name: String,
}

/// 历史记录查询请求（带 oneshot 回复通道）
pub struct StorageQuery {
    pub limit: usize,
    pub reply: tokio::sync::oneshot::Sender<anyhow::Result<Vec<crate::clipboard::storage::HistoryItem>>>,
}

/// 历史记录清空请求（带 oneshot 回复通道）
pub struct StorageClear {
    pub reply: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
}

/// 配对存储操作
pub enum PairingOp {
    Check {
        device_id: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<bool>>,
    },
    Add {
        device_id: String,
        device_name: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
    Remove {
        device_id: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
    List {
        reply: tokio::sync::oneshot::Sender<anyhow::Result<Vec<crate::clipboard::storage::PairedDevice>>>,
    },
    CheckCooldown {
        device_id: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<bool>>,
    },
    SetCooldown {
        device_id: String,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
    /// 新设备请求配对（等待用户审批）
    PendingRequest {
        device_id: String,
        device_name: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },
    /// 用户审批配对请求
    ApproveRequest { device_id: String },
    /// 用户拒绝配对请求
    RejectRequest { device_id: String },
    /// 查询待处理的配对请求列表
    ListPending {
        reply: tokio::sync::oneshot::Sender<Vec<(String, String)>>,
    },
}

/// 应用主协调器
pub struct App {
    /// 应用配置
    config: AppConfig,
    /// 设备身份管理器
    identity: Arc<IdentityManager>,
    /// 加密传输层
    crypto: Arc<CryptoTransport>,
    /// mDNS 服务（使用 RwLock 以支持运行时更新设备名称）
    mdns: Arc<RwLock<MdnsService>>,
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
    /// 历史存储清空发送通道
    storage_clear_tx: mpsc::UnboundedSender<StorageClear>,
    /// 配对操作发送通道
    pairing_tx: mpsc::UnboundedSender<PairingOp>,
    /// 去重服务
    dedup_service: Arc<DeduplicationService>,
    /// 已连接的客户端（设备 ID -> WebSocketClient）
    clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
    /// 已连接的入站远程设备 ID 集合
    server_connected_device_ids: Arc<RwLock<HashSet<String>>>,
    /// WebSocket 预绑定 listener（传递给 run() 保持端口持续监听）
    ws_listener: std::net::TcpListener,
    /// Tauri 应用句柄（用于向前端推送事件）
    app_handle: tauri::AppHandle,
}

/// 应用对外暴露给 IPC 命令使用的句柄（轻量、Send + Sync）
pub struct IpcHandles {
    /// 应用配置
    pub config: Arc<RwLock<AppConfig>>,
    /// 设备身份管理器
    pub identity: Arc<IdentityManager>,
    /// mDNS 服务（用于查询发现的设备列表和广播设备名称）
    pub mdns: Arc<RwLock<MdnsService>>,
    /// 历史存储查询通道
    pub storage_query_tx: mpsc::UnboundedSender<StorageQuery>,
    /// 历史存储清空通道
    pub storage_clear_tx: mpsc::UnboundedSender<StorageClear>,
    /// 配对操作通道
    pub pairing_tx: mpsc::UnboundedSender<PairingOp>,
    /// 已连接的出站 WebSocket 客户端
    pub clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
    /// 已连接的入站远程设备 ID 集合
    pub server_connected_device_ids: Arc<RwLock<HashSet<String>>>,
}

impl IpcHandles {
    /// 更新自定义设备名称并触发 mDNS 广播
    pub async fn set_custom_device_name(&self, name: Option<String>) -> anyhow::Result<()> {
        log::info!("开始更新设备名称: {:?}", name);

        // 1. 更新配置文件
        {
            let mut config = self.config.write().await;
            config.set_custom_device_name(name.clone())?;
            log::info!("配置文件已更新");
        }

        // 2. 如果提供了新名称，触发 mDNS 重新广播
        if let Some(new_name) = name {
            log::info!("触发 mDNS 广播新名称: {}", new_name);
            let mut mdns = self.mdns.write().await;
            mdns.update_device_name(new_name)
                .map_err(|e| {
                    log::error!("mDNS 广播更新失败: {}", e);
                    anyhow::anyhow!("mDNS 广播更新失败: {}", e)
                })?;
            log::info!("✓ mDNS 广播完成");
        }

        Ok(())
    }
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

        // 初始化设备身份（在并行初始化之前，因为 mdns 需要公钥）
        let identity = IdentityManager::new(config.identity_path())?;
        let identity = Arc::new(identity);
        let device_id = identity.device_id().to_string();

        // 同步 config.device_id 为 identity 派生 ID（消除 mDNS 与 WebSocket 的 ID 不一致）
        let mut config = config;
        if config.device_id != device_id {
            info!(
                "同步设备 ID: config={} → identity={}",
                config.device_id, device_id
            );
            config.device_id = device_id.clone();
            if let Err(e) = config.save() {
                warn!("保存同步后的设备 ID 失败: {}", e);
            }
        }

        // 初始化加密传输层
        let crypto = Arc::new(CryptoTransport::new(Arc::clone(&identity)));

        // 创建共享的入站连接设备 ID 集合
        let server_connected_device_ids: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

        // 并行初始化三个独立模块：mDNS（带公钥）、WebSocket Server、Storage
        let mdns_handle = {
            let device_id = device_id.clone();
            let device_name = config.display_name().to_string();
            let port = config.port;
            let public_key_b64 = Some(identity.public_key_base64());
            tokio::spawn(async move {
                MdnsService::new(device_id, device_name, port, public_key_b64, CRYPTO_VERSION.to_string())
            })
        };

        let ws_server_handle = {
            let bind_addr = format!("0.0.0.0:{}", config.port);
            let device_id = device_id.clone();
            let connected_ids = Arc::clone(&server_connected_device_ids);
            let crypto = Arc::clone(&crypto);
            tokio::spawn(async move {
                WebSocketServer::new(bind_addr, device_id, connected_ids, crypto)
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
        let (ws_server, ws_server_rx, ws_listener) = ws_server_handle.await??;
        let history_storage = storage_handle.await??;

        info!("mDNS、WebSocket Server、Storage 并行初始化完成");

        // 注册 mDNS 服务
        mdns.register()?;
        info!("mDNS 服务已注册，端口: {}", mdns.get_port());

        // 初始化粘贴板监听器和写入器
        let (clipboard_monitor, clipboard_rx) = ClipboardMonitor::new(app_handle.clone());
        let clipboard_writer = ClipboardWriter::new(app_handle.clone());

        // 创建去重服务
        let dedup_service = DeduplicationService::new();

        // 创建存储请求通道
        let (storage_tx, storage_rx) = mpsc::unbounded_channel();
        // 创建存储查询通道
        let (storage_query_tx, storage_query_rx) = mpsc::unbounded_channel::<StorageQuery>();
        // 创建存储清空通道
        let (storage_clear_tx, storage_clear_rx) = mpsc::unbounded_channel::<StorageClear>();
        // 创建配对操作通道
        let (pairing_tx, pairing_rx) = mpsc::unbounded_channel::<PairingOp>();

        // 启动存储处理任务（在独立线程中，因为 rusqlite 不是 Send）
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("创建存储任务运行时失败");
            rt.block_on(async move {
                Self::handle_storage_requests(history_storage, storage_rx, storage_query_rx, storage_clear_rx, pairing_rx).await;
            });
        });

        info!("应用初始化完成");

        Ok(Self {
            config,
            identity,
            crypto,
            mdns: Arc::new(RwLock::new(mdns)),
            ws_server: Arc::new(ws_server),
            ws_server_rx: Arc::new(RwLock::new(ws_server_rx)),
            clipboard_monitor,
            clipboard_rx: Arc::new(RwLock::new(clipboard_rx)),
            clipboard_writer: Arc::new(clipboard_writer),
            storage_tx,
            storage_query_tx,
            storage_clear_tx,
            pairing_tx,
            dedup_service: Arc::new(dedup_service),
            clients: Arc::new(RwLock::new(HashMap::new())),
            server_connected_device_ids,
            ws_listener,
            app_handle,
        })
    }

    /// 处理存储请求和查询（在独立线程中运行）
    async fn handle_storage_requests(
        mut storage: HistoryStorage,
        mut storage_rx: mpsc::UnboundedReceiver<StorageRequest>,
        mut storage_query_rx: mpsc::UnboundedReceiver<StorageQuery>,
        mut storage_clear_rx: mpsc::UnboundedReceiver<StorageClear>,
        mut pairing_rx: mpsc::UnboundedReceiver<PairingOp>,
    ) {
        info!("存储处理任务已启动");

        // 存储待审批的配对请求（device_id -> oneshot reply sender）
        let mut pending_requests: HashMap<String, tokio::sync::oneshot::Sender<bool>> = HashMap::new();

        loop {
            tokio::select! {
                Some(req) = storage_rx.recv() => {
                    if let Err(e) = storage.insert(&req.content, &req.content_type, &req.device_id, &req.device_name) {
                        error!("保存历史记录失败: {}", e);
                    }
                }
                Some(query) = storage_query_rx.recv() => {
                    let result = storage
                        .query_recent(query.limit)
                        .map_err(|e| anyhow::anyhow!("查询历史记录失败: {}", e));
                    let _ = query.reply.send(result);
                }
                Some(clear) = storage_clear_rx.recv() => {
                    let result = storage
                        .clear_all()
                        .map_err(|e| anyhow::anyhow!("清空历史记录失败: {}", e));
                    let _ = clear.reply.send(result);
                }
                Some(op) = pairing_rx.recv() => {
                    match op {
                        PairingOp::Check { device_id, reply } => {
                            let result = storage.is_paired(&device_id)
                                .map_err(|e| anyhow::anyhow!("配对检查失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::Add { device_id, device_name, reply } => {
                            let result = storage.add_paired_device(&device_id, &device_name)
                                .map_err(|e| anyhow::anyhow!("添加配对设备失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::Remove { device_id, reply } => {
                            let result = storage.remove_paired_device(&device_id)
                                .map_err(|e| anyhow::anyhow!("移除配对设备失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::List { reply } => {
                            let result = storage.list_paired_devices()
                                .map_err(|e| anyhow::anyhow!("获取配对设备列表失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::CheckCooldown { device_id, reply } => {
                            let result = storage.is_in_cooldown(&device_id)
                                .map_err(|e| anyhow::anyhow!("冷却检查失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::SetCooldown { device_id, reply } => {
                            let result = storage.set_cooldown(&device_id)
                                .map_err(|e| anyhow::anyhow!("设置冷却失败: {}", e));
                            let _ = reply.send(result);
                        }
                        PairingOp::PendingRequest { device_id, device_name: _, reply } => {
                            // 存储待审批请求的 reply sender，等待前端审批
                            info!("存储待审批配对请求: {}", device_id);
                            pending_requests.insert(device_id, reply);
                        }
                        PairingOp::ApproveRequest { device_id } => {
                            // 前端同意配对，通知等待中的请求
                            if let Some(tx) = pending_requests.remove(&device_id) {
                                info!("用户同意配对: {}", device_id);
                                let _ = tx.send(true);
                            } else {
                                warn!("未找到待审批的配对请求: {}", device_id);
                            }
                        }
                        PairingOp::RejectRequest { device_id } => {
                            // 前端拒绝配对，通知等待中的请求
                            if let Some(tx) = pending_requests.remove(&device_id) {
                                info!("用户拒绝配对: {}", device_id);
                                let _ = tx.send(false);
                            } else {
                                warn!("未找到待审批的配对请求: {}", device_id);
                            }
                        }
                        PairingOp::ListPending { reply } => {
                            // 返回待处理的配对请求列表（device_id, device_name）
                            // 注意：我们只有 device_id，device_name 需要从其他地方获取
                            // 这里先返回 device_id，device_name 用 device_id 代替
                            let pending: Vec<(String, String)> = pending_requests
                                .keys()
                                .map(|id| (id.clone(), id.clone()))
                                .collect();
                            let _ = reply.send(pending);
                        }
                    }
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
            identity: _,
            crypto,
            mdns,
            ws_server,
            ws_server_rx,
            clipboard_monitor,
            clipboard_rx,
            clipboard_writer,
            storage_tx,
            storage_query_tx: _, // 已在 Self 中持有，无须再传递
            storage_clear_tx: _, // 已在 Self 中持有，无须再传递
            pairing_tx, // 同时传入 handle_incoming_messages_task
            dedup_service,
            clients,
            server_connected_device_ids,
            ws_listener,
            app_handle,
        } = self;

        // 启动 WebSocket 服务端（独立任务）
        let ws_server_for_run = Arc::clone(&ws_server);
        let ws_listener = ws_listener;  // 移动到闭包
        tokio::spawn(async move {
            if let Err(e) = ws_server_for_run.run(ws_listener).await {
                error!("WebSocket 服务端运行失败: {}", e);
            }
        });

        // 启动 mDNS 监听（使用 spawn_blocking 因为 listen() 是阻塞调用）
        let mdns_for_listen = Arc::clone(&mdns);
        std::thread::spawn(move || {
            let mdns_guard = mdns_for_listen.blocking_read();
            if let Err(e) = mdns_guard.listen() {
                error!("mDNS 监听失败: {}", e);
            }
        });

        // 启动 UDP 广播发现（作为 mDNS 的备用方案，避免 macOS 端口 5353 冲突）
        {
            let mdns_guard = mdns.read().await;
            mdns_guard.start_broadcast_discovery();
        }

        // 启动粘贴板监听器（独立任务）
        tokio::spawn(async move {
            clipboard_monitor.start().await;
        });

        // 启动设备发现到连接建立的流程（独立任务）
        {
            let mdns_for_discovery = Arc::clone(&mdns);
            let config_for_discovery = config.clone();
            let crypto_for_discovery = Arc::clone(&crypto);
            let clients_for_discovery = Arc::clone(&clients);
            let server_ids_for_discovery = Arc::clone(&server_connected_device_ids);
            let dedup_service_for_discovery = Arc::clone(&dedup_service);
            let clipboard_writer_for_discovery = Arc::clone(&clipboard_writer);
            let storage_tx_for_discovery = storage_tx.clone();
            let pairing_tx_for_discovery = pairing_tx.clone();
            let app_handle_for_discovery = app_handle.clone();

            tokio::spawn(async move {
                Self::handle_device_discovery_task(
                    mdns_for_discovery,
                    config_for_discovery,
                    crypto_for_discovery,
                    clients_for_discovery,
                    server_ids_for_discovery,
                    dedup_service_for_discovery,
                    clipboard_writer_for_discovery,
                    storage_tx_for_discovery,
                    pairing_tx_for_discovery,
                    app_handle_for_discovery,
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
            storage_tx.clone(),
            pairing_tx.clone(),
            Arc::clone(&ws_server),
            config.clone(),
            app_handle,
        ).await;

        Ok(())
    }

    /// 处理设备发现到连接建立的流程（静态方法）
    ///
    /// 流程：mDNS 发现设备 → 获取设备信息 → WebSocketClient 连接
    async fn handle_device_discovery_task(
        mdns: Arc<RwLock<MdnsService>>,
        config: AppConfig,
        crypto: Arc<CryptoTransport>,
        clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
        server_connected_ids: Arc<RwLock<HashSet<String>>>,
        dedup_service: Arc<DeduplicationService>,
        clipboard_writer: Arc<ClipboardWriter>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
        pairing_tx: mpsc::UnboundedSender<PairingOp>,
        app_handle: tauri::AppHandle,
    ) {
        info!("设备发现流程已启动");

        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));

        loop {
            ticker.tick().await;

            // 同步已连接的设备到 mDNS 设备列表，防止被清理任务删除
            {
                let clients_read = clients.read().await;
                let mdns_read = mdns.read().await;
                for id in clients_read.keys() {
                    mdns_read.update_device_heartbeat(id);
                }
            }
            {
                let server_read = server_connected_ids.read().await;
                let mdns_read = mdns.read().await;
                for id in server_read.iter() {
                    mdns_read.update_device_heartbeat(id);
                }
            }

            // 获取当前发现的设备列表
            let devices = {
                let mdns_read = mdns.read().await;
                mdns_read.get_devices()
            };
            debug!("设备发现周期: 共发现 {} 台设备", devices.len());

            for device in devices {
                // 跳过已连接的设备
                {
                    let clients_read = clients.read().await;
                    if clients_read.contains_key(&device.id) {
                        continue;
                    }
                }

                // 跳过不兼容的设备（加密版本不匹配）
                if !device.is_compatible() {
                    warn!("设备 {} 加密版本不兼容（本地: v{}, 远程: v{}），跳过连接",
                        device.name, CRYPTO_VERSION, device.crypto_version);
                    continue;
                }
                // 尝试连接到新发现的设备
                info!("发现新设备: {} ({}:{})", device.name, device.addr, device.port);
                Self::connect_to_device(
                    device,
                    config.clone(),
                    Arc::clone(&crypto),
                    Arc::clone(&clients),
                    Arc::clone(&server_connected_ids),
                    Arc::clone(&dedup_service),
                    Arc::clone(&clipboard_writer),
                    storage_tx.clone(),
                    pairing_tx.clone(),
                    app_handle.clone(),
                ).await;
            }
        }
    }

    /// 连接到指定设备（静态方法）
    ///
    /// 检查是否已通过服务器连接（入站连接），避免重复连接。
    /// 如果设备已通过服务器连接，则跳过客户端连接。
    async fn connect_to_device(
        device: DeviceInfo,
        config: AppConfig,
        crypto: Arc<CryptoTransport>,
        clients: Arc<RwLock<HashMap<String, Arc<WebSocketClient>>>>,
        server_connected_ids: Arc<RwLock<HashSet<String>>>,
        dedup_service: Arc<DeduplicationService>,
        clipboard_writer: Arc<ClipboardWriter>,
        storage_tx: mpsc::UnboundedSender<StorageRequest>,
        pairing_tx: mpsc::UnboundedSender<PairingOp>,
        app_handle: tauri::AppHandle,
    ) {
        // 检查是否已通过客户端连接
        {
            let clients_read = clients.read().await;
            if clients_read.contains_key(&device.id) {
                info!("设备 {} 已通过客户端连接，跳过", device.name);
                return;
            }
        }

        // 检查是否已通过服务器连接（入站连接）
        {
            let server_ids = server_connected_ids.read().await;
            if server_ids.contains(&device.id) {
                info!("设备 {} 已通过服务器连接，跳过客户端连接", device.name);
                return;
            }
        }
        
        let server_url = format!("ws://{}:{}", device.addr, device.port);
        let device_id = config.device_id.clone();

        info!("尝试连接到设备 {}: {}", device.name, server_url);

        // 创建 WebSocket 客户端
        let (client, mut client_rx) =
            WebSocketClient::new(server_url.clone(), device_id.clone(), crypto);
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
        let config_for_connect = config.clone();

        tokio::spawn(async move {
            match client_for_connect.connect().await {
                Ok(_) => {
                    info!("成功连接到设备: {}", device_name);

                    // 发送配对请求
                    let pairing_req = Message::new_pairing_request(
                        config_for_connect.device_id.clone(),
                        config_for_connect.device_name.clone(),
                        format!("{:x}", Sha256::digest(config_for_connect.device_id.as_bytes())),
                    );
                    if let Err(e) = client_for_connect.send(pairing_req).await {
                        error!("发送配对请求到 {} 失败: {}", device_name, e);
                    } else {
                        info!("已向设备 {} 发送配对请求", device_name);
                    }

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
        let config_for_rx = config.clone();
        let client_for_rx_send = Arc::clone(&client);

        tokio::spawn(async move {
            while let Some(msg) = client_rx.recv().await {
                // ── 配对响应处理 ──
                if msg.is_pairing_response() {
                    let remote_id = msg.device_id().to_string();
                    let accepted = msg.pairing_accepted().unwrap_or(false);
                    let reason = msg.pairing_reason().flatten();

                    if accepted {
                        info!("设备 {} 已接受配对请求，开始同步粘贴板", device_name);
                        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                        let _ = pairing_tx.send(PairingOp::Add {
                            device_id: remote_id.clone(),
                            device_name: device_name.clone(),
                            reply: reply_tx,
                        });
                        if let Ok(Ok(_)) = reply_rx.await {
                            info!("设备 {} 已添加至本地配对列表", device_name);
                        }
                    } else {
                        warn!("设备 {} 拒绝配对: {:?}", device_name, reason);
                    }
                    continue;
                }

                // ── 配对请求处理（对方主动发起配对）──
                if msg.is_pairing_request() {
                    let remote_id = msg.device_id().to_string();
                    let remote_name = msg.pairing_device_name().unwrap_or(&remote_id).to_string();

                    // 检查是否已配对
                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                    let _ = pairing_tx.send(PairingOp::Check { device_id: remote_id.clone(), reply: reply_tx });

                    let is_paired = reply_rx.await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or(false);

                    if is_paired {
                        info!("设备 {} 已配对，自动接受", remote_name);
                        let response = Message::new_pairing_response(
                            config_for_rx.device_id.clone(),
                            true,
                            None,
                        );
                        let _ = client_for_rx_send.send(response).await;
                    } else {
                        info!("设备 {} 未配对，等待用户审批", remote_name);

                        // 通过 Tauri 事件通知前端弹出审批弹窗
                        let _ = app_handle.emit("pairing-request", serde_json::json!({
                            "device_id": remote_id,
                            "device_name": remote_name,
                        }));

                        // 通过 PairingOp channel 等待用户审批
                        let (approve_tx, approve_rx) = tokio::sync::oneshot::channel();
                        let _ = pairing_tx.send(PairingOp::PendingRequest {
                            device_id: remote_id.clone(),
                            device_name: remote_name.clone(),
                            reply: approve_tx,
                        });

                        // 60 秒超时自动拒绝
                        let approved = match tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            approve_rx,
                        ).await {
                            Ok(Ok(true)) => {
                                info!("用户同意配对: {}", remote_name);
                                true
                            }
                            _ => {
                                warn!("配对审批超时或失败，自动拒绝: {}", remote_name);
                                false
                            }
                        };

                        if approved {
                            // 添加到配对列表
                            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                            let _ = pairing_tx.send(PairingOp::Add {
                                device_id: remote_id.clone(),
                                device_name: remote_name.clone(),
                                reply: reply_tx,
                            });
                            let _ = reply_rx.await;

                            // 回复同意
                            let response = Message::new_pairing_response(
                                config_for_rx.device_id.clone(),
                                true,
                                None,
                            );
                            let _ = client_for_rx_send.send(response).await;
                        } else {
                            // 回复拒绝
                            let response = Message::new_pairing_response(
                                config_for_rx.device_id.clone(),
                                false,
                                Some("用户拒绝配对".to_string()),
                            );
                            let _ = client_for_rx_send.send(response).await;
                        }
                    }
                    continue;
                }

                // ── 粘贴板消息处理 ──
                if msg.is_clipboard() {
                    // 配对检查：仅处理已配对设备的消息
                    let remote_id = msg.device_id().to_string();
                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                    let _ = pairing_tx.send(PairingOp::Check { device_id: remote_id.clone(), reply: reply_tx });

                    let is_paired = reply_rx.await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or(false);

                    if !is_paired {
                        debug!("设备 {} 未配对，跳过其粘贴板消息", device_name);
                        continue;
                    }

                    if let (Some(content), Some(uuid)) = (msg.content(), msg.uuid()) {
                        // 计算内容哈希
                        let content_hash = Self::calculate_hash(content);

                        // 去重检查
                        if !dedup_service.should_process_message(uuid, &content_hash).await {
                            continue;
                        }

                        // 写入粘贴板
                        match clipboard_writer.write(content.to_string(), msg.content_type().to_string(), uuid.to_string()).await {
                            Ok(true) => {
                                info!("收到来自 {} 的粘贴板内容，已写入本地", device_name);

                                // 标记消息已处理
                                dedup_service.mark_message_processed(uuid.to_string(), content_hash).await;

                                // 发送存储请求
                                let _ = storage_tx.send(StorageRequest {
                                    content: content.to_string(),
                                    content_type: msg.content_type().to_string(),
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
                    continue;
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
                change.content_type.clone(),
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
                content_type: change.content_type,
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
        pairing_tx: mpsc::UnboundedSender<PairingOp>,
        ws_server: Arc<WebSocketServer>,
        config: AppConfig,
        app_handle: tauri::AppHandle,
    ) {
        info!("消息接收处理流程已启动");

        let mut ws_server_rx = ws_server_rx.write().await;

        while let Some(msg) = ws_server_rx.recv().await {
            // ── 配对请求处理 ──
            if msg.is_pairing_request() {
                let remote_id = msg.device_id().to_string();
                let remote_name = msg.pairing_device_name().unwrap_or(&remote_id).to_string();
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                let _ = pairing_tx.send(PairingOp::Check { device_id: remote_id.clone(), reply: reply_tx });

                let is_paired = reply_rx.await
                    .ok()
                    .and_then(|r| r.ok())
                    .unwrap_or(false);

                if is_paired {
                    info!("设备 {} 已配对，自动接受", remote_name);
                    let response = Message::new_pairing_response(
                        config.device_id.clone(),
                        true,
                        None,
                    );
                    let _ = ws_server.broadcast(&response).await;
                } else {
                    info!("设备 {} 未配对，等待用户审批", remote_name);

                    // 通过 Tauri 事件通知前端弹出审批弹窗
                    let _ = app_handle.emit("pairing-request", serde_json::json!({
                        "device_id": remote_id,
                        "device_name": remote_name,
                    }));

                    // 通过 PairingOp channel 等待用户审批
                    let (approve_tx, approve_rx) = tokio::sync::oneshot::channel();
                    let _ = pairing_tx.send(PairingOp::PendingRequest {
                        device_id: remote_id.clone(),
                        device_name: remote_name.clone(),
                        reply: approve_tx,
                    });

                    // 60 秒超时自动拒绝
                    let approved = match tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        approve_rx,
                    ).await {
                        Ok(Ok(true)) => {
                            info!("用户同意配对: {}", remote_name);
                            true
                        }
                        _ => {
                            warn!("配对审批超时或失败，自动拒绝: {}", remote_name);
                            false
                        }
                    };

                    if approved {
                        // 添加到配对列表
                        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                        let _ = pairing_tx.send(PairingOp::Add {
                            device_id: remote_id.clone(),
                            device_name: remote_name.clone(),
                            reply: reply_tx,
                        });
                        let _ = reply_rx.await;

                        // 回复同意
                        let response = Message::new_pairing_response(
                            config.device_id.clone(),
                            true,
                            None,
                        );
                        let _ = ws_server.broadcast(&response).await;
                    } else {
                        // 回复拒绝
                        let response = Message::new_pairing_response(
                            config.device_id.clone(),
                            false,
                            Some("用户拒绝配对".to_string()),
                        );
                        let _ = ws_server.broadcast(&response).await;
                    }
                }
                continue;
            }

            // ── 配对响应处理 ──
            if msg.is_pairing_response() {
                let remote_id = msg.device_id().to_string();
                let accepted = msg.pairing_accepted().unwrap_or(false);
                let reason = msg.pairing_reason().flatten();

                info!("收到设备 {} 的配对响应: accepted={}, reason={:?}", remote_id, accepted, reason);

                if accepted {
                    // 需要 device_name 来添加配对记录，但从响应消息中拿不到设备名
                    // 从 mDNS 匹配（响应消息的 device_id 即对方唯一标识）
                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                    let _ = pairing_tx.send(PairingOp::Add {
                        device_id: remote_id.clone(),
                        device_name: remote_id.clone(),
                        reply: reply_tx,
                    });
                    if let Ok(Ok(_)) = reply_rx.await {
                        info!("设备 {} 已添加至配对列表", remote_id);
                    }
                }
                continue;
            }

            // ── 粘贴板消息处理 ──
            if msg.is_clipboard() {
                // 配对检查：仅处理已配对设备的消息
                let remote_id = msg.device_id().to_string();
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                let _ = pairing_tx.send(PairingOp::Check { device_id: remote_id.clone(), reply: reply_tx });

                let is_paired = reply_rx.await
                    .ok()
                    .and_then(|r| r.ok())
                    .unwrap_or(false);

                if !is_paired {
                    debug!("设备 {} 未配对，跳过其粘贴板消息", remote_id);
                    continue;
                }

                if let (Some(content), Some(uuid)) = (msg.content(), msg.uuid()) {
                    // 计算内容哈希
                    let content_hash = Self::calculate_hash(content);

                    // 去重检查
                    if !dedup_service.should_process_message(uuid, &content_hash).await {
                        continue;
                    }

                    // 写入粘贴板
                    match clipboard_writer.write(content.to_string(), msg.content_type().to_string(), uuid.to_string()).await {
                        Ok(true) => {
                            info!("收到已配对设备 {} 的粘贴板内容，已写入本地，内容长度: {} 字节", remote_id, content.len());

                            // 标记消息已处理
                            dedup_service.mark_message_processed(uuid.to_string(), content_hash).await;

                            // 发送存储请求
                            let _ = storage_tx.send(StorageRequest {
                                content: content.to_string(),
                                content_type: msg.content_type().to_string(),
                                device_id: remote_id,
                                device_name: "Unknown Device".to_string(),
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
            config: Arc::new(RwLock::new(self.config.clone())),
            identity: Arc::clone(&self.identity),
            mdns: Arc::clone(&self.mdns),
            storage_query_tx: self.storage_query_tx.clone(),
            storage_clear_tx: self.storage_clear_tx.clone(),
            pairing_tx: self.pairing_tx.clone(),
            clients: Arc::clone(&self.clients),
            server_connected_device_ids: Arc::clone(&self.server_connected_device_ids),
        }
    }

    /// 获取当前已发现设备列表快照（供 IPC 使用）
    pub async fn get_devices_snapshot(&self) -> Vec<DeviceSnapshot> {
        let mdns = self.mdns.read().await;
        mdns.get_devices()
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

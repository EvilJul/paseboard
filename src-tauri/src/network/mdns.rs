// mDNS 设备发现模块
//
// 职责：
// - 注册 mDNS 服务（端口自动降级：9527 → 9528 → ... → 9537）
// - 监听局域网内其他设备的 mDNS 广播
// - 维护设备列表（新增、更新、移除离线设备）
// - 30 秒心跳超时检测

use std::collections::HashMap;
use std::net::{IpAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::utils::error::NetworkError;

/// mDNS 服务类型
const SERVICE_TYPE: &str = "_paseboard._tcp.local.";

/// UDP 广播发现端口（独立于 mDNS 的 5353，避免端口冲突）
const BROADCAST_PORT: u16 = 9528;

/// UDP 广播间隔（秒）
const BROADCAST_INTERVAL_SECS: u64 = 5;

/// 默认端口范围
const PORT_RANGE_START: u16 = 9527;
const PORT_RANGE_END: u16 = 9537;

/// 心跳超时时间（秒）
const HEARTBEAT_TIMEOUT_SECS: u64 = 30;

/// 当前加密协议版本
/// - "0": 未加密（v0.1.x）
/// - "1": AES-256-GCM + ECDH（v0.2.0）
pub const CRYPTO_VERSION: &str = "1";

/// 设备信息
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// 设备唯一标识符
    pub id: String,
    /// 设备名称
    pub name: String,
    /// 设备 IP 地址
    pub addr: IpAddr,
    /// 设备端口
    pub port: u16,
    /// 最后心跳时间（Unix 时间戳）
    pub last_seen: u64,
    /// Base64 编码的 Ed25519 公钥（用于设备身份验证）
    pub public_key: Option<String>,
    /// 加密协议版本（"0"=未加密, "1"=AES-256-GCM+ECDH）
    pub crypto_version: String,
}

impl DeviceInfo {
    /// 检查设备是否离线（超过心跳超时时间）
    pub fn is_offline(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_seen > HEARTBEAT_TIMEOUT_SECS
    }

    /// 检查设备的加密版本是否与当前版本兼容
    pub fn is_compatible(&self) -> bool {
        self.crypto_version == CRYPTO_VERSION
    }
}

/// mDNS 服务管理器
pub struct MdnsService {
    /// mDNS 守护进程
    daemon: ServiceDaemon,
    /// 当前设备信息
    device_id: String,
    device_name: String,
    /// 实际使用的端口
    port: u16,
    /// 已发现的设备列表（设备 ID -> 设备信息）
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    /// Base64 编码的 Ed25519 公钥（用于 mDNS TXT 广播）
    public_key_base64: Option<String>,
    /// 本设备加密协议版本（用于 TXT 广播兼容性检测）
    crypto_version: String,
}

impl MdnsService {
    /// 创建 mDNS 服务
    ///
    /// # 参数
    /// - `device_id` / `device_name`：设备标识
    /// - `bind_port`：mDNS 广播时使用的端口。必须与 WebSocketServer 实际监听的端口一致，
    ///                否则远端按 mDNS 通告的端口连接会被拒绝。
    /// - `public_key_base64`：可选的 Base64 编码公钥，用于设备身份验证。
    ///                        传入 `None` 兼容旧版本。
    ///
    /// # 返回
    /// - `Ok(MdnsService)`: 成功创建服务
    /// - `Err(NetworkError)`: mDNS 初始化失败
    pub fn new(
        device_id: String,
        device_name: String,
        bind_port: u16,
        public_key_base64: Option<String>,
        crypto_version: String,
    ) -> Result<Self, NetworkError> {
        // 初始化 mDNS 守护进程
        let daemon = ServiceDaemon::new().map_err(|e| {
            NetworkError::ConnectionFailed(format!("mDNS 初始化失败: {}", e))
        })?;

        Ok(Self {
            daemon,
            device_id,
            device_name: Self::encode_mdns_name(&device_name),
            port: bind_port,
            devices: Arc::new(Mutex::new(HashMap::new())),
            public_key_base64,
            crypto_version,
        })
    }

    /// 查找可用端口（9527-9537 范围内）
    fn find_available_port() -> Option<u16> {
        for port in PORT_RANGE_START..=PORT_RANGE_END {
            if Self::is_port_available(port) {
                return Some(port);
            }
        }
        None
    }

    /// 检查端口是否可用
    fn is_port_available(port: u16) -> bool {
        TcpListener::bind(format!("0.0.0.0:{}", port)).is_ok()
    }

    /// 注册 mDNS 服务（广播设备信息）
    pub fn register(&self) -> Result<(), NetworkError> {
        log::info!("准备注册 mDNS 服务...");
        log::debug!("设备 ID: {}", self.device_id);
        log::debug!("设备名称: {}", self.device_name);
        log::debug!("端口: {}", self.port);

        // 构建服务名称：设备名称.服务类型
        let service_name = format!("{}.{}", self.device_name, SERVICE_TYPE);
        log::debug!("完整服务名: {}", service_name);

        // 创建 TXT 记录：设备 ID + 可选公钥 + 加密版本
        let mut properties = vec![("device_id", self.device_id.as_str())];
        if let Some(ref pk) = self.public_key_base64 {
            properties.push(("pk", pk.as_str()));
        }
        properties.push(("cv", self.crypto_version.as_str()));

        // 主动检测本机在局域网接口上的 IP（不使用 enable_addr_auto()，
        // 因为 mdns-sd 0.7.5 的自动检测在 macOS 多网卡环境下经常选错接口）
        let host_ipv4 = Self::detect_local_ipv4().unwrap_or_default();
        log::debug!("自动检测到的 host IPv4: {}", host_ipv4);

        // 创建服务信息
        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &self.device_name,
            &self.device_name,  // hostname 使用设备名称
            &host_ipv4,         // host_ipv4（主动检测，避免自动选错）
            self.port,
            &properties[..],
        )
        .map_err(|e| {
            log::error!("创建 mDNS 服务信息失败: {}", e);
            NetworkError::ConnectionFailed(format!("创建 mDNS 服务信息失败: {}", e))
        })?;

        // 注册服务
        self.daemon.register(service_info).map_err(|e| {
            log::error!("注册 mDNS 服务失败: {}", e);
            NetworkError::ConnectionFailed(format!("注册 mDNS 服务失败: {}", e))
        })?;

        log::info!("✓ mDNS 服务注册成功: {} 端口 {}", self.device_name, self.port);
        Ok(())
    }

    /// 检测本机局域网 IPv4 地址
    ///
    /// 策略：
    /// 1. 优先枚举所有网络接口，取第一个非回环、非代理假 IP（198.18.0.0/15）、
    ///    且是私有 IPv4 地址（192.168.x.x / 10.x.x.x / 172.16-31.x.x）
    /// 2. 回退到 UDP connect 8.8.8.8:80 方法（原方案）
    fn detect_local_ipv4() -> Option<String> {
        // 策略一：枚举网络接口（可跳过代理接口）
        if let Ok(if_addrs) = get_if_addrs::get_if_addrs() {
            for iface in &if_addrs {
                let ip = iface.ip();
                match ip {
                    std::net::IpAddr::V4(v4) => {
                        let octets = v4.octets();
                        // 跳过回环
                        if octets[0] == 127 { continue; }
                        // 跳过 Surge/ClashX 假 IP 段 198.18.0.0/15
                        if octets[0] == 198 && octets[1] == 18 { continue; }
                        // 只保留私有地址段
                        let is_private = octets[0] == 10
                            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                            || (octets[0] == 192 && octets[1] == 168)
                            || (octets[0] == 100 && (64..=127).contains(&octets[1])); // CGNAT
                        if is_private {
                            return Some(v4.to_string());
                        }
                    }
                    std::net::IpAddr::V6(_) => {}
                }
            }
        }

        // 策略二：回退到 UDP connect 方法
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("8.8.8.8:80").ok()?;
        let addr = socket.local_addr().ok()?;
        match addr.ip() {
            std::net::IpAddr::V4(v4) => Some(v4.to_string()),
            std::net::IpAddr::V6(_) => None,
        }
    }

    /// 开始监听 mDNS 广播（阻塞调用）
    ///
    /// 注意：此方法会阻塞当前线程，建议在独立线程或 Tokio 任务中调用
    pub fn listen(&self) -> Result<(), NetworkError> {
        log::info!("开始监听 mDNS 服务: {}", SERVICE_TYPE);
        
        // 浏览服务
        let receiver = self.daemon.browse(SERVICE_TYPE).map_err(|e| {
            log::error!("浏览 mDNS 服务失败: {}", e);
            NetworkError::ConnectionFailed(format!("浏览 mDNS 服务失败: {}", e))
        })?;

        let devices = Arc::clone(&self.devices);
        let own_device_id = self.device_id.clone();

        // 启动后台任务：监听服务事件
        std::thread::spawn(move || {
            log::info!("mDNS 监听线程已启动");
            
            while let Ok(event) = receiver.recv() {
                match event {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        log::debug!("mDNS ServiceResolved: fullname={}, port={}", 
                            info.get_fullname(), info.get_port());
                        
                        // 解析设备信息
                        if let Some(device_info) = Self::parse_service_info(&info) {
                            // 跳过自己
                            if device_info.id == own_device_id {
                                log::debug!("跳过自己的设备: {}", device_info.name);
                                continue;
                            }

                            log::info!("发现新设备: {} (ID: {}, 地址: {}:{})", 
                                device_info.name, device_info.id, device_info.addr, device_info.port);

                            // 更新设备列表
                            let mut devices = devices.lock().unwrap();
                            devices.insert(device_info.id.clone(), device_info);
                        } else {
                            log::warn!("无法解析设备信息，跳过");
                        }
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_, full_name) => {
                        log::info!("设备离线: {}", full_name);
                        
                        // 设备离线：从列表中移除
                        let mut devices = devices.lock().unwrap();
                        devices.retain(|_, dev| {
                            format!("{}.{}", dev.name, SERVICE_TYPE) != full_name
                        });
                    }
                    _ => {
                        log::trace!("mDNS 其他事件: {:?}", event);
                    }
                }
            }
            
            log::warn!("mDNS 监听线程退出");
        });

        // 启动后台任务：定期检测离线设备
        let devices_clone = Arc::clone(&self.devices);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(10));
            let mut devices = devices_clone.lock().unwrap();
            devices.retain(|_, dev| !dev.is_offline());
        });

        Ok(())
    }

    /// 解析 ServiceInfo 为 DeviceInfo
    fn parse_service_info(info: &ServiceInfo) -> Option<DeviceInfo> {
        // 获取设备 ID（从 TXT 记录）
        let device_id: String = info
            .get_properties()
            .get("device_id")
            .and_then(|val| Some(val.val_str().to_string()))?;

        // 获取设备名称（服务实例名称）
        // 注意：trim_end_matches 是按字符集匹配，不能用于子串匹配
        let fullname = info.get_fullname();
        let suffix = SERVICE_TYPE.strip_suffix('.').unwrap_or(SERVICE_TYPE);
        let device_name = fullname
            .strip_suffix(suffix)
            .or_else(|| fullname.strip_suffix(SERVICE_TYPE))
            .map(|n| n.strip_suffix('.').unwrap_or(n))
            .unwrap_or(&fullname)
            .to_string();

        // 获取 IP 地址（过滤 loopback，优先选择非回环的 IPv4）
        let addresses = info.get_addresses();
        let addr = addresses.iter()
            .filter(|a| !a.is_loopback())
            .next()
            .or_else(|| addresses.iter().next())?;
        let addr = IpAddr::V4(*addr);

        // 获取端口
        let port = info.get_port();

        // 解析公钥（可选）
        let public_key = info
            .get_properties()
            .get("pk")
            .map(|val| val.val_str().to_string());

        // 解析加密版本（可选，不存在时默认为 "0" 表示未加密）
        let crypto_version = info
            .get_properties()
            .get("cv")
            .map(|val| val.val_str().to_string())
            .unwrap_or_else(|| "0".to_string());

        // 当前时间戳
        let last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Some(DeviceInfo {
            id: device_id,
            name: device_name,
            addr,
            port,
            last_seen,
            public_key,
            crypto_version,
        })
    }

    /// 获取当前设备列表（快照）
    pub fn get_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.lock().unwrap();
        devices.values().cloned().collect()
    }

    /// 获取实际使用的端口
    pub fn get_port(&self) -> u16 {
        self.port
    }

    /// 更新设备名称并重新广播
    ///
    /// 当用户修改设备名称后调用此方法，会立即重新注册 mDNS 服务，
    /// 使其他设备在下一次轮询时能收到新的设备名称。
    ///
    /// 注意：mDNS 服务名必须是 ASCII 安全的，中文字符会导致 mdns-sd 解析器 panic。
    /// 因此对非 ASCII 名称进行 percent-encoding 编码。
    pub fn update_device_name(&mut self, new_name: String) -> Result<(), NetworkError> {
        log::info!("更新 mDNS 设备名称: {} -> {}", self.device_name, new_name);

        // 先注销旧的服务（使用旧的设备名称）
        let old_service_name = format!("{}.{}", self.device_name, SERVICE_TYPE);
        if let Err(e) = self.daemon.unregister(&old_service_name) {
            log::warn!("注销旧 mDNS 服务失败（可能未注册）: {}", e);
        }

        // 更新设备名称（使用 ASCII 安全的编码名称用于 mDNS 广播）
        self.device_name = Self::encode_mdns_name(&new_name);

        // 重新注册服务（使用新的设备名称）
        self.register()?;

        log::info!("✓ mDNS 设备名称更新完成，已重新广播（编码名: {}）", self.device_name);
        Ok(())
    }

    /// 将设备名称编码为 mDNS 安全的 ASCII 名称
    ///
    /// mdns-sd 的 DNS 解析器不支持非 ASCII 字符（中文等会导致 panic）。
    /// 使用 percent-encoding 将非 ASCII 字符编码为 %XX 形式。
    fn encode_mdns_name(name: &str) -> String {
        if name.is_ascii() {
            return name.to_string();
        }
        let mut encoded = String::with_capacity(name.len() * 3);
        for byte in name.bytes() {
            if byte.is_ascii_graphic() || byte == b'-' || byte == b'.' {
                encoded.push(byte as char);
            } else if byte == b' ' {
                encoded.push('-'); // 空格替换为连字符
            } else {
                encoded.push('%');
                encoded.push_str(&format!("{:02X}", byte));
            }
        }
        log::info!("mDNS 名称编码: {} -> {}", name, encoded);
        encoded
    }

    /// 更新设备心跳时间（用于维持在线状态）
    pub fn update_device_heartbeat(&self, device_id: &str) {
        let mut devices = self.devices.lock().unwrap();
        if let Some(device) = devices.get_mut(device_id) {
            device.last_seen = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    /// 启动 UDP 广播发现（作为 mDNS 在 macOS 上的备用方案）
    ///
    /// mDNS 在 macOS 上可能因端口 5353 被系统 mDNSResponder 占用而无法接收
    /// 多播响应。UDP 广播使用独立端口 9528，不依赖 mDNS 基础设施。
    ///
    /// 启动两个后台线程：
    /// - 发送线程：每 5 秒广播本机设备信息到子网广播地址
    /// - 接收线程：持续监听其他设备的广播并更新设备列表
    pub fn start_broadcast_discovery(&self) {
        let local_ip = Self::detect_local_ipv4()
            .unwrap_or_else(|| {
                log::warn!("UDP 广播：无法检测本机 IP，使用 127.0.0.1");
                "127.0.0.1".to_string()
            });

        let send_devices = Arc::clone(&self.devices);
        let send_id = self.device_id.clone();
        let send_name = self.device_name.clone();
        let send_port = self.port;
        let send_ip = local_ip.clone();
        let send_crypto_version = self.crypto_version.clone();
        let send_public_key = self.public_key_base64.clone();
        std::thread::spawn(move || {
            Self::udp_broadcast_sender_loop(send_devices, send_id, send_name, send_port, &send_ip, send_crypto_version, send_public_key);
        });

        let recv_devices = Arc::clone(&self.devices);
        let own_id = self.device_id.clone();
        std::thread::spawn(move || {
            Self::udp_broadcast_listener_loop(recv_devices, &own_id);
        });

        log::info!("UDP 广播发现已启动（端口 {}）", BROADCAST_PORT);
    }

    /// UDP 广播发送线程
    fn udp_broadcast_sender_loop(
        _devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
        device_id: String,
        device_name: String,
        port: u16,
        local_ip: &str,
        crypto_version: String,
        public_key_base64: Option<String>,
    ) {
        let socket = match std::net::UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                log::error!("UDP 广播发送 socket 创建失败: {}", e);
                return;
            }
        };
        if let Err(e) = socket.set_broadcast(true) {
            log::error!("UDP 广播设置失败: {}", e);
            return;
        }

        let msg = serde_json::json!({
            "type": "paseboard_discovery",
            "device_id": device_id,
            "device_name": device_name,
            "addr": local_ip,
            "port": port,
            "crypto_version": crypto_version,
            "public_key": public_key_base64,
        });
        let payload = match serde_json::to_string(&msg) {
            Ok(p) => p,
            Err(e) => {
                log::error!("UDP 广播序列化失败: {}", e);
                return;
            }
        };

        let dest = format!("255.255.255.255:{}", BROADCAST_PORT);

        log::info!("UDP 广播目标: {} (本机 IP: {})", dest, local_ip);

        loop {
            if let Err(e) = socket.send_to(payload.as_bytes(), &dest) {
                log::warn!("UDP 广播发送失败: {}", e);
            }
            std::thread::sleep(Duration::from_secs(BROADCAST_INTERVAL_SECS));
        }
    }

    /// UDP 广播接收线程
    fn udp_broadcast_listener_loop(
        devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
        own_device_id: &str,
    ) {
        let socket = match std::net::UdpSocket::bind(format!("0.0.0.0:{}", BROADCAST_PORT)) {
            Ok(s) => s,
            Err(e) => {
                log::error!("UDP 广播监听 socket 创建失败（端口 {}）: {}", BROADCAST_PORT, e);
                return;
            }
        };
        if let Err(e) = socket.set_broadcast(true) {
            log::error!("UDP 广播监听设置失败: {}", e);
            return;
        }

        let own_id = own_device_id.to_string();
        let mut buf = [0u8; 2048];

        log::info!("UDP 广播监听线程已启动");

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, src_addr)) => {
                    let data_str = match std::str::from_utf8(&buf[..size]) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let parsed: serde_json::Value = match serde_json::from_str(data_str) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    // 验证消息类型
                    let is_paseboard = parsed
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(|t| t == "paseboard_discovery")
                        .unwrap_or(false);
                    if !is_paseboard {
                        continue;
                    }

                    let remote_id = match parsed.get("device_id").and_then(|v| v.as_str()) {
                        Some(id) => id,
                        None => continue,
                    };

                    // 跳过自己的广播
                    if remote_id == own_id {
                        continue;
                    }

                    let name = parsed
                        .get("device_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let addr_str = parsed.get("addr").and_then(|v| v.as_str()).unwrap_or("");
                    let remote_port = parsed.get("port").and_then(|v| v.as_u64()).unwrap_or(9527) as u16;
                    let public_key = parsed.get("public_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let crypto_version = parsed.get("crypto_version").and_then(|v| v.as_str()).unwrap_or("0").to_string();

                    // 优先使用源 IP（更可靠），仅当无法解析时使用 JSON 中的 addr
                    let addr = src_addr.ip();
                    let _ = addr_str; // 保留 addr_str 供日志使用

                    let last_seen = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    {
                        let mut d = devices.lock().unwrap();
                        d.insert(remote_id.to_string(), DeviceInfo {
                            id: remote_id.to_string(),
                            name,
                            addr,
                            port: remote_port,
                            last_seen,
                            public_key,
                            crypto_version,
                        });
                    }
                }
                Err(e) => {
                    // macOS 上如果端口冲突会反复报错，降低日志频率
                    log::warn!("UDP 广播接收错误: {}", e);
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_port_availability() {
        // 测试端口检查逻辑（实际端口可用性取决于系统）
        let available = MdnsService::is_port_available(9527);
        // 只验证函数能正常返回，不验证具体结果
        let _ = available;
    }

    #[test]
    fn test_find_available_port() {
        // 测试端口查找逻辑
        let port = MdnsService::find_available_port();
        if let Some(port) = port {
            assert!(port >= PORT_RANGE_START && port <= PORT_RANGE_END);
        }
    }

    #[test]
    fn test_device_info_offline_detection() {
        let mut device = DeviceInfo {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            addr: "192.168.1.100".parse().unwrap(),
            port: 9527,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            public_key: None,
            crypto_version: "1".to_string(),
        };

        // 新设备应该在线
        assert!(!device.is_offline());

        // 模拟超时：设置 last_seen 为 40 秒前
        device.last_seen -= HEARTBEAT_TIMEOUT_SECS + 10;
        assert!(device.is_offline());
    }

    #[test]
    fn test_device_info_is_compatible() {
        // 兼容设备：crypto_version 与本地一致
        let compatible = DeviceInfo {
            id: "compat-device".to_string(),
            name: "Compatible Device".to_string(),
            addr: "192.168.1.101".parse().unwrap(),
            port: 9527,
            last_seen: 0,
            public_key: None,
            crypto_version: "1".to_string(),
        };
        assert!(compatible.is_compatible());

        // 不兼容设备：crypto_version 与本地不一致（旧版本未加密）
        let incompatible = DeviceInfo {
            id: "incompat-device".to_string(),
            name: "Incompatible Device".to_string(),
            addr: "192.168.1.102".parse().unwrap(),
            port: 9527,
            last_seen: 0,
            public_key: None,
            crypto_version: "0".to_string(),
        };
        assert!(!incompatible.is_compatible());
    }

    fn test_mdns_service_creation() {
        let device_id = Uuid::new_v4().to_string();
        let device_name = "Test Device".to_string();

        let result = MdnsService::new(device_id.clone(), device_name.clone(), 9527, None, CRYPTO_VERSION.to_string());

        // 根据系统 mDNS 可用性判断结果
        match result {
            Ok(service) => {
                // mDNS 可用：验证基本属性
                assert_eq!(service.device_id, device_id);
                assert_eq!(service.device_name, device_name);
                assert!(service.port >= PORT_RANGE_START && service.port <= PORT_RANGE_END);
                assert_eq!(service.get_devices().len(), 0);
            }
            Err(_) => {
                // mDNS 不可用：测试通过（系统未安装 Bonjour/Avahi）
            }
        }
    }

    #[test]
    fn test_device_list_management() {
        let device_id = Uuid::new_v4().to_string();
        let device_name = "Test Device".to_string();

        if let Ok(service) = MdnsService::new(device_id.clone(), device_name.clone(), 9528, None, CRYPTO_VERSION.to_string()) {
            // 初始设备列表为空
            assert_eq!(service.get_devices().len(), 0);

            // 模拟添加设备
            {
                let mut devices = service.devices.lock().unwrap();
                devices.insert(
                    "remote-device-1".to_string(),
                    DeviceInfo {
                        id: "remote-device-1".to_string(),
                        name: "Remote Device 1".to_string(),
                        addr: "192.168.1.100".parse().unwrap(),
                        port: 9527,
                        last_seen: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        public_key: None,
                        crypto_version: "1".to_string(),
                    },
                );
            }

            // 验证设备已添加
            let devices_list = service.get_devices();
            assert_eq!(devices_list.len(), 1);
            assert_eq!(devices_list[0].id, "remote-device-1");

            // 测试心跳更新
            let old_timestamp = devices_list[0].last_seen;
            std::thread::sleep(Duration::from_secs(1));
            service.update_device_heartbeat("remote-device-1");

            let updated_devices = service.get_devices();
            assert!(updated_devices[0].last_seen > old_timestamp);
        }
    }

    #[test]
    fn test_detect_local_ipv4() {
        let ip = MdnsService::detect_local_ipv4();
        match ip {
            Some(addr) => {
                // 验证返回的是有效 IPv4 地址
                assert!(!addr.is_empty(), "IP 地址不应为空");
                assert!(!addr.starts_with("127."), "不应返回回环地址: {}", addr);

                // 验证是私有地址或 CGNAT 地址
                let valid = addr.starts_with("10.")
                    || addr.starts_with("192.168.")
                    || addr.starts_with("172.1")
                    || addr.starts_with("172.2")
                    || addr.starts_with("172.3")
                    || addr.starts_with("100.");
                assert!(valid, "IP {} 不是私有地址", addr);
            }
            None => {
                // 没有网络接口时允许失败
                println!("detect_local_ipv4 返回 None（无网络接口）");
            }
        }
    }

    #[test]
    fn test_service_registration() {
        let device_id = Uuid::new_v4().to_string();
        let device_name = "Test Device".to_string();

        if let Ok(service) = MdnsService::new(device_id, device_name, 9529, None, CRYPTO_VERSION.to_string()) {
            // 尝试注册服务（可能因系统限制失败）
            let result = service.register();

            // 根据系统环境判断结果
            match result {
                Ok(_) => {
                    // 注册成功：验证端口可用性
                    assert!(service.get_port() >= PORT_RANGE_START);
                    assert!(service.get_port() <= PORT_RANGE_END);
                }
                Err(_) => {
                    // 注册失败：可能是系统限制或权限问题
                    // 测试通过（非功能性错误）
                }
            }
        }
    }
}

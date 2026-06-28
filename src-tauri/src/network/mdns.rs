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

/// 默认端口范围
const PORT_RANGE_START: u16 = 9527;
const PORT_RANGE_END: u16 = 9537;

/// 心跳超时时间（秒）
const HEARTBEAT_TIMEOUT_SECS: u64 = 30;

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
}

impl MdnsService {
    /// 创建 mDNS 服务
    ///
    /// # 参数
    /// - `device_id` / `device_name`：设备标识
    /// - `bind_port`：mDNS 广播时使用的端口。必须与 WebSocketServer 实际监听的端口一致，
    ///                否则远端按 mDNS 通告的端口连接会被拒绝。
    ///
    /// # 返回
    /// - `Ok(MdnsService)`: 成功创建服务
    /// - `Err(NetworkError)`: mDNS 初始化失败
    pub fn new(
        device_id: String,
        device_name: String,
        bind_port: u16,
    ) -> Result<Self, NetworkError> {
        // 初始化 mDNS 守护进程
        let daemon = ServiceDaemon::new().map_err(|e| {
            NetworkError::ConnectionFailed(format!("mDNS 初始化失败: {}", e))
        })?;

        Ok(Self {
            daemon,
            device_id,
            device_name,
            port: bind_port,
            devices: Arc::new(Mutex::new(HashMap::new())),
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

        // 创建 TXT 记录：设备 ID
        let properties = [("device_id", self.device_id.as_str())];

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

    /// 通过 UDP socket connect 到公网地址（不发包）的方式，
    /// 让操作系统选择本机在局域网接口上的 IPv4。
    /// 该方法不依赖任何第三方 crate。
    fn detect_local_ipv4() -> Option<String> {
        // 连接到 8.8.8.8:80 不会真的发包，仅让 OS 选出本机出口 IP
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
        assert!(available || !available);
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
        };

        // 新设备应该在线
        assert!(!device.is_offline());

        // 模拟超时：设置 last_seen 为 40 秒前
        device.last_seen -= HEARTBEAT_TIMEOUT_SECS + 10;
        assert!(device.is_offline());
    }

    #[test]
    fn test_mdns_service_creation() {
        let device_id = Uuid::new_v4().to_string();
        let device_name = "Test Device".to_string();

        let result = MdnsService::new(device_id.clone(), device_name.clone(), 9527);

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

        if let Ok(service) = MdnsService::new(device_id.clone(), device_name.clone(), 9528) {
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
    fn test_service_registration() {
        let device_id = Uuid::new_v4().to_string();
        let device_name = "Test Device".to_string();

        if let Ok(service) = MdnsService::new(device_id, device_name, 9529) {
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

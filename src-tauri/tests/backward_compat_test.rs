// 向后兼容性测试
//
// 测试 mDNS 设备发现中的加密版本兼容性检测

use paseboard::network::mdns::{DeviceInfo, CRYPTO_VERSION};

#[test]
fn test_compatible_device() {
    let device = DeviceInfo {
        id: "compat-device".to_string(),
        name: "Compatible Device".to_string(),
        addr: "192.168.1.100".parse().unwrap(),
        port: 9527,
        last_seen: 0,
        public_key: None,
        crypto_version: CRYPTO_VERSION.to_string(),
    };
    assert!(device.is_compatible(), "加密版本一致的设备应被判定为兼容");
}

#[test]
fn test_incompatible_device_old_version() {
    let device = DeviceInfo {
        id: "old-device".to_string(),
        name: "Old Device".to_string(),
        addr: "192.168.1.101".parse().unwrap(),
        port: 9527,
        last_seen: 0,
        public_key: None,
        crypto_version: "0".to_string(),
    };
    assert!(!device.is_compatible(), "旧版本（v0）设备应被判定为不兼容");
}

#[test]
fn test_incompatible_device_unknown_version() {
    let device = DeviceInfo {
        id: "unknown-device".to_string(),
        name: "Unknown Device".to_string(),
        addr: "192.168.1.102".parse().unwrap(),
        port: 9527,
        last_seen: 0,
        public_key: None,
        crypto_version: "999".to_string(),
    };
    assert!(!device.is_compatible(), "未知加密版本的设备应被判定为不兼容");
}

#[test]
fn test_incompatible_device_no_cv_field() {
    // 没有 cv 字段的旧设备，默认 crypto_version 为 "0"
    let device = DeviceInfo {
        id: "no-cv-device".to_string(),
        name: "No CV Device".to_string(),
        addr: "192.168.1.103".parse().unwrap(),
        port: 9527,
        last_seen: 0,
        public_key: None,
        crypto_version: "0".to_string(),
    };
    assert!(!device.is_compatible(), "无 cv 字段的旧设备默认为 v0，应判定为不兼容");
}

#[test]
fn test_crypto_version_constant() {
    // 确保 CRYPTO_VERSION 不为空且格式正确
    assert!(!CRYPTO_VERSION.is_empty(), "CRYPTO_VERSION 不能为空");
    let version: u32 = CRYPTO_VERSION.parse().expect("CRYPTO_VERSION 应为数字字符串");
    assert!(version > 0, "CRYPTO_VERSION 应大于 0");
}

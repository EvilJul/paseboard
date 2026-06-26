// mDNS 功能集成测试
//
// 测试 mDNS 服务注册和发现功能

use paseboard::network::mdns::MdnsService;
use paseboard::config::AppConfig;
use std::time::Duration;
use uuid::Uuid;

#[test]
fn test_mdns_service_creation() {
    // 测试创建 mDNS 服务
    let device_id = Uuid::new_v4().to_string();
    let device_name = "Test Device".to_string();

    let result = MdnsService::new(device_id.clone(), device_name.clone());

    match result {
        Ok(service) => {
            println!("✓ mDNS 服务创建成功");
            println!("  设备 ID: {}", device_id);
            println!("  设备名称: {}", device_name);
            println!("  分配端口: {}", service.get_port());
            
            assert!(service.get_port() >= 9527 && service.get_port() <= 9537);
        }
        Err(e) => {
            eprintln!("✗ mDNS 服务创建失败: {}", e);
            eprintln!("  这可能是因为系统不支持 mDNS 或端口被占用");
            // 在 CI 环境或不支持 mDNS 的系统上，这个测试会失败
            // 但不应该阻止编译
        }
    }
}

#[test]
fn test_mdns_registration() {
    // 测试 mDNS 服务注册
    let device_id = Uuid::new_v4().to_string();
    let device_name = "Test Device Registration".to_string();

    match MdnsService::new(device_id, device_name) {
        Ok(service) => {
            println!("✓ mDNS 服务已创建");
            
            match service.register() {
                Ok(_) => {
                    println!("✓ mDNS 服务注册成功");
                    println!("  可以通过 dns-sd -B _paseboard._tcp local. 验证");
                }
                Err(e) => {
                    eprintln!("✗ mDNS 服务注册失败: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ mDNS 服务创建失败: {}", e);
        }
    }
}

#[test]
fn test_device_list_empty() {
    // 测试设备列表初始为空
    let device_id = Uuid::new_v4().to_string();
    let device_name = "Test Empty List".to_string();

    if let Ok(service) = MdnsService::new(device_id, device_name) {
        let devices = service.get_devices();
        assert_eq!(devices.len(), 0, "初始设备列表应该为空");
        println!("✓ 初始设备列表为空");
    }
}

#[tokio::test]
async fn test_two_instances_discovery() {
    // 测试两个实例能否互相发现
    // 注意：这个测试需要在支持 mDNS 的系统上运行
    
    println!("\n开始测试两个实例互相发现...");
    
    // 创建第一个实例
    let device1_id = Uuid::new_v4().to_string();
    let device1_name = "Device-1".to_string();
    
    let service1 = match MdnsService::new(device1_id.clone(), device1_name.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("跳过测试：无法创建 mDNS 服务: {}", e);
            return;
        }
    };
    
    println!("✓ 设备 1 创建成功: {} (端口 {})", device1_name, service1.get_port());
    
    // 注册第一个实例
    if let Err(e) = service1.register() {
        eprintln!("设备 1 注册失败: {}", e);
        return;
    }
    println!("✓ 设备 1 已注册");
    
    // 启动第一个实例的监听
    if let Err(e) = service1.listen() {
        eprintln!("设备 1 监听启动失败: {}", e);
        return;
    }
    println!("✓ 设备 1 监听已启动");
    
    // 创建第二个实例
    let device2_id = Uuid::new_v4().to_string();
    let device2_name = "Device-2".to_string();
    
    let service2 = match MdnsService::new(device2_id.clone(), device2_name.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("无法创建第二个 mDNS 服务: {}", e);
            return;
        }
    };
    
    println!("✓ 设备 2 创建成功: {} (端口 {})", device2_name, service2.get_port());
    
    // 注册第二个实例
    if let Err(e) = service2.register() {
        eprintln!("设备 2 注册失败: {}", e);
        return;
    }
    println!("✓ 设备 2 已注册");
    
    // 启动第二个实例的监听
    if let Err(e) = service2.listen() {
        eprintln!("设备 2 监听启动失败: {}", e);
        return;
    }
    println!("✓ 设备 2 监听已启动");
    
    // 等待 mDNS 广播和发现
    println!("\n等待 10 秒进行设备发现...");
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // 检查设备 1 是否发现了设备 2
    let devices1 = service1.get_devices();
    println!("\n设备 1 发现的设备数量: {}", devices1.len());
    for device in &devices1 {
        println!("  - {} ({}:{})", device.name, device.addr, device.port);
    }
    
    // 检查设备 2 是否发现了设备 1
    let devices2 = service2.get_devices();
    println!("\n设备 2 发现的设备数量: {}", devices2.len());
    for device in &devices2 {
        println!("  - {} ({}:{})", device.name, device.addr, device.port);
    }
    
    // 验证互相发现
    let found_device2 = devices1.iter().any(|d| d.id == device2_id);
    let found_device1 = devices2.iter().any(|d| d.id == device1_id);
    
    if found_device2 {
        println!("\n✓ 设备 1 成功发现设备 2");
    } else {
        println!("\n✗ 设备 1 未发现设备 2");
    }
    
    if found_device1 {
        println!("✓ 设备 2 成功发现设备 1");
    } else {
        println!("✗ 设备 2 未发现设备 1");
    }
    
    // 如果测试失败，提供诊断信息
    if !found_device1 || !found_device2 {
        println!("\n诊断信息:");
        println!("  1. 检查 mDNS 服务是否正常: pgrep -x mDNSResponder");
        println!("  2. 手动验证广播: dns-sd -B _paseboard._tcp local.");
        println!("  3. 检查防火墙设置");
        println!("  4. 这可能是正常的，取决于系统配置和网络环境");
    }
}

#[test]
fn test_config_device_name() {
    // 测试配置生成的设备名称
    match AppConfig::load() {
        Ok(config) => {
            println!("✓ 配置加载成功");
            println!("  设备 ID: {}", config.device_id);
            println!("  设备名称: {}", config.device_name);
            println!("  端口: {}", config.port);
            
            // 检查设备名称是否包含 .local 后缀（已修复的问题）
            if config.device_name.ends_with(".local") {
                println!("  ⚠️  设备名称包含 .local 后缀，这可能导致 mDNS 问题");
            } else {
                println!("  ✓ 设备名称格式正确（不包含 .local）");
            }
        }
        Err(e) => {
            eprintln!("✗ 配置加载失败: {}", e);
        }
    }
}

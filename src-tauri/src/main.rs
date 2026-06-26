// 禁止未使用代码警告（开发阶段）
#![allow(dead_code)]
// 隐藏 Windows 平台的控制台窗口
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod config;
mod network;
mod clipboard;
mod utils;
mod app;

use config::AppConfig;
use app::App;
use log::info;
use tauri::Manager;

/// 设备信息（用于 IPC 返回）
#[derive(Debug, Clone, serde::Serialize)]
struct DeviceInfo {
    id: String,
    name: String,
    addr: String,
    port: u16,
    last_seen: u64,
    is_online: bool,
}

/// 历史记录项（用于 IPC 返回）
#[derive(Debug, Clone, serde::Serialize)]
struct HistoryItem {
    id: i64,
    content: String,
    device_id: String,
    device_name: String,
    timestamp: i64,
    size: i64,
}

/// 查询设备列表 IPC 命令
#[tauri::command]
async fn get_devices() -> Result<Vec<DeviceInfo>, String> {
    // TODO: 从 App 实例获取设备列表
    // 当前返回空列表，后续需要通过全局状态访问 mdns.get_devices()
    Ok(vec![])
}

/// 查询历史记录 IPC 命令（最近 100 条）
#[tauri::command]
async fn get_history() -> Result<Vec<HistoryItem>, String> {
    // TODO: 从 HistoryStorage 查询历史记录
    // 当前返回空列表，后续需要通过全局状态访问 storage.get_recent(100)
    Ok(vec![])
}

fn main() {
    // 初始化日志系统
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("PaseBoard 启动中...");

    // 加载应用配置
    let config = match AppConfig::load() {
        Ok(cfg) => {
            info!("配置加载成功: 设备 ID = {}, 设备名称 = {}, 端口 = {}",
                cfg.device_id, cfg.device_name, cfg.port);
            cfg
        }
        Err(e) => {
            eprintln!("加载配置失败: {}", e);
            std::process::exit(1);
        }
    };

    // 启动 Tauri 应用
    tauri::Builder::default()
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // 关闭窗口时最小化到托盘，不退出应用
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_devices, get_history])
        .setup(move |app| {
            // 创建系统托盘
            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        // 点击托盘图标显示主窗口
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let app_handle = app.handle().clone();

            // 在独立的 Tokio 运行时中启动应用逻辑
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("创建 Tokio 运行时失败");
                rt.block_on(async move {
                    // 创建应用实例
                    let app = match App::new(config, app_handle).await {
                        Ok(app) => app,
                        Err(e) => {
                            eprintln!("应用初始化失败: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // 运行应用主循环
                    if let Err(e) = app.run().await {
                        eprintln!("应用运行失败: {}", e);
                        std::process::exit(1);
                    }
                });
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}

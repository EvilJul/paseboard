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
use app::{App, DeviceSnapshot, IpcHandles};
use log::info;
use std::sync::Arc;
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

impl From<DeviceSnapshot> for DeviceInfo {
    fn from(d: DeviceSnapshot) -> Self {
        let is_online = !d.is_offline();
        Self {
            id: d.id,
            name: d.name,
            addr: d.addr,
            port: d.port,
            last_seen: d.last_seen,
            is_online,
        }
    }
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

impl From<crate::clipboard::storage::HistoryItem> for HistoryItem {
    fn from(item: crate::clipboard::storage::HistoryItem) -> Self {
        Self {
            id: item.id,
            content: item.content,
            device_id: item.device_id,
            device_name: item.device_name,
            timestamp: item.timestamp,
            size: item.size,
        }
    }
}

/// 全局应用句柄（用于在 IPC 命令中访问已发现的设备和历史记录）
pub type AppState = Arc<IpcHandles>;

/// 查询设备列表 IPC 命令
#[tauri::command]
async fn get_devices(state: tauri::State<'_, AppState>) -> Result<Vec<DeviceInfo>, String> {
    let snapshots: Vec<DeviceSnapshot> = state.mdns.get_devices().into_iter().map(Into::into).collect();
    Ok(snapshots.into_iter().map(DeviceInfo::from).collect())
}

/// 查询历史记录 IPC 命令（最近 100 条）
#[tauri::command]
async fn get_history(state: tauri::State<'_, AppState>) -> Result<Vec<HistoryItem>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .storage_query_tx
        .send(app::StorageQuery {
            limit: 100,
            reply: tx,
        })
        .map_err(|_| "存储查询通道已关闭".to_string())?;
    let items = rx
        .await
        .map_err(|_| "存储查询响应失败".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(items.into_iter().map(HistoryItem::from).collect())
}

/// 打开开发者工具（仅 debug 模式生效；release 下打印日志便于诊断）
#[tauri::command]
fn open_devtools(window: tauri::Window) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        if let Some(ww) = window.get_webview_window("main") {
            ww.open_devtools();
        }
    }
    info!("open_devtools 调用（仅 debug 模式有效）");
    let _ = window;
    Ok(())
}

fn main() {
    // 初始化日志系统
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("PaseBoard 启动中...");

    // 加载应用配置
    let config = match AppConfig::load() {
        Ok(cfg) => {
            info!(
                "配置加载成功: 设备 ID = {}, 设备名称 = {}, 端口 = {}",
                cfg.device_id, cfg.device_name, cfg.port
            );
            cfg
        }
        Err(e) => {
            eprintln!("加载配置失败: {}", e);
            std::process::exit(1);
        }
    };

    // 启动 Tauri 应用
    tauri::Builder::default()
        // 注册 tauri-plugin-clipboard-manager，否则前端
        // `plugin:clipboard-manager|write_text` 调用会失败
        .plugin(tauri_plugin_clipboard_manager::init())
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // 关闭窗口时最小化到托盘，不退出应用
                window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_devices,
            get_history,
            open_devtools
        ])
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
                    let app = match App::new(config, app_handle.clone()).await {
                        Ok(app) => app,
                        Err(e) => {
                            eprintln!("应用初始化失败: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // 仅将 IPC 命令真正需要的句柄注册到 Tauri 全局状态
                    // （避免 App 中非 Send/Sync 字段导致状态注册失败）
                    let ipc_handles = Arc::new(app.ipc_handles());
                    app_handle.manage(ipc_handles);

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

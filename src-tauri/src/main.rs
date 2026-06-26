// 禁止未使用代码警告（开发阶段）
#![allow(dead_code)]

mod config;
mod network;
mod clipboard;
mod utils;

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// 网络通信模块
//
// 职责：
// - mDNS 设备发现
// - WebSocket 服务端和客户端
// - 消息协议定义与编解码

pub mod mdns;
pub mod message;
pub mod websocket_common;
pub mod websocket_server;
pub mod websocket_client;

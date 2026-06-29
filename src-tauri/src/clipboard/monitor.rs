// 粘贴板监听器
//
// 职责：
// - 以 500ms 间隔轮询系统粘贴板
// - 计算内容 SHA256 哈希值
// - 检测内容变化并触发事件
// - 过滤来自网络的内容（防止回环）

use crate::utils::error::ClipboardError;
use sha2::{Digest, Sha256};
use arboard::Clipboard;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

/// 粘贴板内容最大大小（10MB）
const MAX_CONTENT_SIZE: usize = 10 * 1024 * 1024;

/// 轮询间隔（500ms）
const POLL_INTERVAL_MS: u64 = 500;

/// 粘贴板变化事件
#[derive(Debug, Clone)]
pub struct ClipboardChange {
    /// 粘贴板内容
    pub content: String,
    /// 内容类型（"text" 或 "image"）
    pub content_type: String,
    /// 内容哈希（SHA256）
    pub hash: String,
}

/// 粘贴板监听器
pub struct ClipboardMonitor {
    /// 上次检查的内容哈希
    last_hash: Option<String>,
    /// 事件发送通道
    tx: mpsc::UnboundedSender<ClipboardChange>,
}

impl ClipboardMonitor {
    /// 创建新的监听器
    ///
    /// # 参数
    /// - `app_handle`: Tauri 应用句柄（Tauri 2.x 中不再使用）
    ///
    /// # 返回
    /// - 监听器实例和事件接收通道
    pub fn new(_app_handle: tauri::AppHandle) -> (Self, mpsc::UnboundedReceiver<ClipboardChange>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let monitor = Self {
            last_hash: None,
            tx,
        };

        (monitor, rx)
    }

    /// 启动监听循环
    ///
    /// 该方法会阻塞当前任务，持续轮询粘贴板内容
    pub async fn start(mut self) {
        log::info!("粘贴板监听器已启动，轮询间隔: {}ms", POLL_INTERVAL_MS);

        let mut ticker = interval(Duration::from_millis(POLL_INTERVAL_MS));

        loop {
            ticker.tick().await;

            if let Err(e) = self.check_clipboard().await {
                log::error!("粘贴板检查失败: {}", e);
            }
        }
    }

    /// 检查粘贴板内容变化
    async fn check_clipboard(&mut self) -> Result<(), ClipboardError> {
        // 读取粘贴板内容
        let (content, content_type) = match self.read_clipboard().await {
            Ok(c) => c,
            Err(e) => {
                // 粘贴板可能被锁定或为空，这是正常情况
                log::trace!("读取粘贴板失败（可能为空或被锁定）: {}", e);
                return Ok(());
            }
        };

        // 检查内容大小
        if content.len() > MAX_CONTENT_SIZE {
            log::warn!(
                "粘贴板内容过大: {} 字节超过限制 {} 字节，跳过同步",
                content.len(),
                MAX_CONTENT_SIZE
            );
            return Ok(());
        }

        // 计算内容哈希
        let hash = Self::calculate_hash(&content);

        // 检查是否与上次相同
        if let Some(last_hash) = &self.last_hash {
            if last_hash == &hash {
                // 内容未变化，跳过
                return Ok(());
            }
        }

        log::debug!("检测到粘贴板内容变化，哈希: {}", hash);

        // 检查是否来自网络（通过元数据标记判断）
        if self.is_from_network().await {
            log::debug!("内容来自网络，跳过推送（防止回环）");
            // 更新哈希，但不触发事件
            self.last_hash = Some(hash);
            return Ok(());
        }

        // 更新最后哈希
        self.last_hash = Some(hash.clone());

        // 发送变化事件
        let change = ClipboardChange {
            content,
            content_type,
            hash,
        };

        if let Err(e) = self.tx.send(change) {
            log::error!("发送粘贴板变化事件失败: {}", e);
        }

        Ok(())
    }

    /// 读取粘贴板内容
    async fn read_clipboard(&self) -> Result<(String, String), ClipboardError> {
        // 使用 arboard 读取剪贴板内容
        tokio::task::spawn_blocking(move || {
            let mut clipboard = Clipboard::new()
                .map_err(|e| ClipboardError::ClipboardLocked(format!("创建剪贴板实例失败: {}", e)))?;

            // 先尝试读取文本
            match clipboard.get_text() {
                Ok(text) if !text.is_empty() => return Ok((text, "text".to_string())),
                _ => {} // 文本为空或读取失败，继续尝试图片
            }

            // 尝试读取图片
            match clipboard.get_image() {
                Ok(img_data) => {
                    let png_bytes = Self::rgba_to_png(
                        &img_data.bytes,
                        img_data.width,
                        img_data.height,
                    )?;
                    let base64_str = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &png_bytes,
                    );
                    Ok((base64_str, "image".to_string()))
                }
                Err(e) => Err(ClipboardError::ClipboardLocked(format!(
                    "读取粘贴板失败（文本和图片均不可用）: {}",
                    e
                ))),
            }
        })
        .await
        .map_err(|e| ClipboardError::ClipboardLocked(format!("任务执行失败: {}", e)))?
    }

    /// RGBA 原始数据编码为 PNG 格式的 bytes
    fn rgba_to_png(rgba: &[u8], width: usize, height: usize) -> Result<Vec<u8>, ClipboardError> {
        use image::{ImageBuffer, RgbaImage};
        let img: RgbaImage = ImageBuffer::from_raw(width as u32, height as u32, rgba.to_vec())
            .ok_or_else(|| ClipboardError::ClipboardLocked("创建图片缓冲区失败".to_string()))?;
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| ClipboardError::ClipboardLocked(format!("PNG 编码失败: {}", e)))?;
        Ok(buf.into_inner())
    }

    /// 检查内容是否来自网络
    ///
    /// 实现方式：检查特殊的元数据标记
    /// 注意：arboard 不支持自定义元数据，
    /// 这里先返回 false，实际去重由 UUID 缓存处理
    async fn is_from_network(&self) -> bool {
        // TODO: arboard 不支持自定义剪贴板元数据
        // 暂时返回 false，依赖 UUID 去重机制防止回环
        false
    }

    /// 计算内容的 SHA256 哈希
    fn calculate_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_hash() {
        let content = "Hello, World!";
        let hash = ClipboardMonitor::calculate_hash(content);

        // SHA256("Hello, World!")
        assert_eq!(
            hash,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_calculate_hash_empty() {
        let content = "";
        let hash = ClipboardMonitor::calculate_hash(content);

        // SHA256("")
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_calculate_hash_consistency() {
        let content = "Test content";
        let hash1 = ClipboardMonitor::calculate_hash(content);
        let hash2 = ClipboardMonitor::calculate_hash(content);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_calculate_hash_different_content() {
        let hash1 = ClipboardMonitor::calculate_hash("content1");
        let hash2 = ClipboardMonitor::calculate_hash("content2");

        assert_ne!(hash1, hash2);
    }
}

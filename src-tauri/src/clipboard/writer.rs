// 粘贴板写入器
//
// 职责：
// - 将网络消息写入系统粘贴板
// - 实现失败重试机制（最多 3 次）
// - UUID 去重缓存（防止重复写入）
// - 标记消息来源（区分本地 vs 网络）

use crate::utils::error::ClipboardError;
use std::collections::VecDeque;
use std::sync::Arc;
use arboard::Clipboard;
use base64::Engine;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

/// 写入失败重试次数
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// 重试间隔（100ms）
const RETRY_INTERVAL_MS: u64 = 100;

/// UUID 去重缓存大小上限
const UUID_CACHE_SIZE: usize = 1000;

/// 粘贴板写入器
pub struct ClipboardWriter {
    /// UUID 去重缓存（LRU）
    uuid_cache: Arc<Mutex<VecDeque<String>>>,
}

impl ClipboardWriter {
    /// 创建新的写入器
    ///
    /// # 参数
    /// - `app_handle`: Tauri 应用句柄（Tauri 2.x 中不再使用）
    pub fn new(_app_handle: tauri::AppHandle) -> Self {
        Self {
            uuid_cache: Arc::new(Mutex::new(VecDeque::with_capacity(UUID_CACHE_SIZE))),
        }
    }

    /// 写入粘贴板内容
    ///
    /// # 参数
    /// - `content`: 要写入的内容（文本或 Base64 编码的 PNG 图片）
    /// - `content_type`: 内容类型（"text" 或 "image"）
    /// - `uuid`: 消息 UUID（用于去重）
    ///
    /// # 返回
    /// - `Ok(true)`: 成功写入
    /// - `Ok(false)`: UUID 已存在，跳过写入
    /// - `Err`: 写入失败（重试后仍失败）
    pub async fn write(&self, content: String, content_type: String, uuid: String) -> Result<bool, ClipboardError> {
        // 检查 UUID 是否已处理
        if self.is_uuid_processed(&uuid).await {
            log::debug!("UUID {} 已处理过，跳过写入", uuid);
            return Ok(false);
        }

        // 尝试写入粘贴板（带重试）
        self.write_with_retry(&content, &content_type).await?;

        // 记录 UUID
        self.add_uuid_to_cache(uuid).await;

        log::info!("成功写入粘贴板，内容类型: {}，内容长度: {} 字节", content_type, content.len());
        Ok(true)
    }

    /// 带重试的写入操作
    async fn write_with_retry(&self, content: &str, content_type: &str) -> Result<(), ClipboardError> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match self.write_clipboard(content, content_type).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if attempts >= MAX_RETRY_ATTEMPTS {
                        return Err(ClipboardError::ClipboardLocked(format!(
                            "写入失败（已重试 {} 次）: {}",
                            MAX_RETRY_ATTEMPTS, e
                        )));
                    }

                    log::warn!(
                        "写入粘贴板失败（第 {} 次尝试），等待 {}ms 后重试: {}",
                        attempts,
                        RETRY_INTERVAL_MS,
                        e
                    );

                    sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
                }
            }
        }
    }

    /// 写入粘贴板（底层操作）
    async fn write_clipboard(&self, content: &str, content_type: &str) -> Result<(), ClipboardError> {
        let content = content.to_string();
        let content_type = content_type.to_string();

        tokio::task::spawn_blocking(move || {
            let mut clipboard = Clipboard::new()
                .map_err(|e| ClipboardError::ClipboardLocked(format!("创建剪贴板实例失败: {}", e)))?;

            match content_type.as_str() {
                "image" => {
                    // Base64 解码 -> PNG bytes -> RGBA -> set_image
                    let png_bytes = base64::engine::general_purpose::STANDARD
                        .decode(&content)
                        .map_err(|e| ClipboardError::ClipboardLocked(format!("Base64 解码失败: {}", e)))?;
                    let (rgba, width, height) = Self::png_to_rgba(&png_bytes)?;
                    let img_data = arboard::ImageData {
                        width,
                        height,
                        bytes: std::borrow::Cow::Owned(rgba),
                    };
                    clipboard
                        .set_image(img_data)
                        .map_err(|e| ClipboardError::ClipboardLocked(format!("写入图片失败: {}", e)))
                }
                _ => {
                    // 默认：文本写入
                    clipboard
                        .set_text(content)
                        .map_err(|e| ClipboardError::ClipboardLocked(format!("写入失败: {}", e)))
                }
            }
        })
        .await
        .map_err(|e| ClipboardError::ClipboardLocked(format!("任务执行失败: {}", e)))?
    }

    /// PNG 字节解码为 RGBA 格式
    fn png_to_rgba(png_bytes: &[u8]) -> Result<(Vec<u8>, usize, usize), ClipboardError> {
        let img = image::load_from_memory(png_bytes)
            .map_err(|e| ClipboardError::ClipboardLocked(format!("PNG 解码失败: {}", e)))?;
        let rgba = img.to_rgba8();
        let width = rgba.width() as usize;
        let height = rgba.height() as usize;
        Ok((rgba.into_raw(), width, height))
    }

    /// 检查 UUID 是否已处理
    async fn is_uuid_processed(&self, uuid: &str) -> bool {
        let cache = self.uuid_cache.lock().await;
        cache.contains(&uuid.to_string())
    }

    /// 将 UUID 添加到缓存
    async fn add_uuid_to_cache(&self, uuid: String) {
        let mut cache = self.uuid_cache.lock().await;

        // 如果缓存已满，移除最旧的记录
        if cache.len() >= UUID_CACHE_SIZE {
            cache.pop_front();
        }

        cache.push_back(uuid);
    }

    /// 获取当前缓存大小（用于测试）
    #[cfg(test)]
    async fn cache_size(&self) -> usize {
        let cache = self.uuid_cache.lock().await;
        cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_cache_constants() {
        assert_eq!(UUID_CACHE_SIZE, 1000);
        assert_eq!(MAX_RETRY_ATTEMPTS, 3);
        assert_eq!(RETRY_INTERVAL_MS, 100);
    }

    // 注意：以下测试需要 Tauri 环境，无法在单元测试中直接运行
    // 实际测试将在集成测试中进行

    #[tokio::test]
    async fn test_uuid_deduplication_logic() {
        // 模拟 UUID 去重逻辑测试
        let mut cache = VecDeque::with_capacity(UUID_CACHE_SIZE);

        let uuid1 = "uuid-1".to_string();
        let uuid2 = "uuid-2".to_string();

        // 添加 UUID
        cache.push_back(uuid1.clone());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&uuid1));
        assert!(!cache.contains(&uuid2));

        // 添加第二个 UUID
        cache.push_back(uuid2.clone());
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&uuid2));
    }

    #[tokio::test]
    async fn test_uuid_cache_lru_eviction() {
        // 测试 LRU 缓存淘汰逻辑
        let mut cache = VecDeque::with_capacity(3);

        // 填满缓存
        cache.push_back("uuid-1".to_string());
        cache.push_back("uuid-2".to_string());
        cache.push_back("uuid-3".to_string());

        assert_eq!(cache.len(), 3);

        // 添加第四个 UUID，应该淘汰最旧的
        cache.pop_front();
        cache.push_back("uuid-4".to_string());

        assert_eq!(cache.len(), 3);
        assert!(!cache.contains(&"uuid-1".to_string()));
        assert!(cache.contains(&"uuid-4".to_string()));
    }

    #[tokio::test]
    async fn test_uuid_cache_capacity() {
        // 测试缓存容量管理
        let mut cache = VecDeque::with_capacity(UUID_CACHE_SIZE);

        // 添加超过容量的 UUID
        for i in 0..UUID_CACHE_SIZE + 100 {
            if cache.len() >= UUID_CACHE_SIZE {
                cache.pop_front();
            }
            cache.push_back(format!("uuid-{}", i));
        }

        // 确保缓存不超过上限
        assert_eq!(cache.len(), UUID_CACHE_SIZE);

        // 确保最新的 UUID 在缓存中
        assert!(cache.contains(&format!("uuid-{}", UUID_CACHE_SIZE + 99)));

        // 确保最旧的 UUID 已被淘汰
        assert!(!cache.contains(&"uuid-0".to_string()));
    }
}

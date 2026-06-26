// 消息去重服务
//
// 职责：
// - 集成 UUID 去重和内容哈希去重
// - 实现双重保险验证逻辑
// - 管理去重缓存（1000 条上限）
// - 防止消息回环

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// UUID 去重缓存大小上限
const UUID_CACHE_SIZE: usize = 1000;

/// 内容哈希去重缓存大小上限
const HASH_CACHE_SIZE: usize = 1000;

/// 消息去重服务
///
/// 提供两层去重机制：
/// 1. UUID 去重：防止同一消息被重复处理（接收端）
/// 2. 内容哈希去重：防止相同内容被重复推送（发送端）
///
/// 双重保险：
/// - UUID 失效时，内容哈希兜底
/// - 内容哈希碰撞时，UUID 兜底
#[derive(Clone)]
pub struct DeduplicationService {
    /// UUID 去重缓存（LRU）
    uuid_cache: Arc<Mutex<VecDeque<String>>>,
    /// 内容哈希去重缓存（LRU）
    hash_cache: Arc<Mutex<VecDeque<String>>>,
}

impl DeduplicationService {
    /// 创建新的去重服务
    pub fn new() -> Self {
        Self {
            uuid_cache: Arc::new(Mutex::new(VecDeque::with_capacity(UUID_CACHE_SIZE))),
            hash_cache: Arc::new(Mutex::new(VecDeque::with_capacity(HASH_CACHE_SIZE))),
        }
    }

    /// 判断是否应该处理消息（接收端）
    ///
    /// 使用双重验证：
    /// 1. 检查 UUID 是否已处理（主要防护）
    /// 2. 检查内容哈希是否已见过（兜底防护）
    ///
    /// # 参数
    /// - `uuid`: 消息唯一标识
    /// - `content_hash`: 内容哈希值
    ///
    /// # 返回
    /// - `true`: 应该处理该消息
    /// - `false`: 消息重复，应跳过
    pub async fn should_process_message(&self, uuid: &str, content_hash: &str) -> bool {
        // 第一层：UUID 去重（主要防护）
        if self.is_uuid_processed(uuid).await {
            log::debug!("UUID {} 已处理过，跳过消息（UUID 去重生效）", uuid);
            return false;
        }

        // 第二层：内容哈希去重（兜底防护）
        if self.is_hash_seen(content_hash).await {
            log::debug!(
                "内容哈希 {} 已见过，跳过消息（内容哈希去重生效，UUID 去重未生效）",
                content_hash
            );
            return false;
        }

        // 两层检查都通过，应该处理消息
        true
    }

    /// 判断是否应该发送消息（发送端）
    ///
    /// 仅使用内容哈希去重，防止相同内容被重复推送
    ///
    /// # 参数
    /// - `content_hash`: 内容哈希值
    ///
    /// # 返回
    /// - `true`: 应该发送该消息
    /// - `false`: 内容重复，应跳过
    pub async fn should_send_message(&self, content_hash: &str) -> bool {
        if self.is_hash_seen(content_hash).await {
            log::debug!("内容哈希 {} 已见过，跳过推送", content_hash);
            return false;
        }

        true
    }

    /// 标记消息已处理（接收端）
    ///
    /// 同时记录 UUID 和内容哈希
    ///
    /// # 参数
    /// - `uuid`: 消息唯一标识
    /// - `content_hash`: 内容哈希值
    pub async fn mark_message_processed(&self, uuid: String, content_hash: String) {
        self.add_uuid(uuid).await;
        self.add_hash(content_hash).await;
    }

    /// 标记消息已发送（发送端）
    ///
    /// 仅记录内容哈希
    ///
    /// # 参数
    /// - `content_hash`: 内容哈希值
    pub async fn mark_message_sent(&self, content_hash: String) {
        self.add_hash(content_hash).await;
    }

    /// 检查 UUID 是否已处理
    async fn is_uuid_processed(&self, uuid: &str) -> bool {
        let cache = self.uuid_cache.lock().await;
        cache.contains(&uuid.to_string())
    }

    /// 检查内容哈希是否已见过
    async fn is_hash_seen(&self, hash: &str) -> bool {
        let cache = self.hash_cache.lock().await;
        cache.contains(&hash.to_string())
    }

    /// 添加 UUID 到缓存
    async fn add_uuid(&self, uuid: String) {
        let mut cache = self.uuid_cache.lock().await;

        // 如果缓存已满，移除最旧的记录（FIFO）
        if cache.len() >= UUID_CACHE_SIZE {
            let removed = cache.pop_front();
            log::trace!("UUID 缓存已满，移除最旧记录: {:?}", removed);
        }

        cache.push_back(uuid);
        log::trace!("UUID 缓存大小: {}/{}", cache.len(), UUID_CACHE_SIZE);
    }

    /// 添加内容哈希到缓存
    async fn add_hash(&self, hash: String) {
        let mut cache = self.hash_cache.lock().await;

        // 如果缓存已满，移除最旧的记录（FIFO）
        if cache.len() >= HASH_CACHE_SIZE {
            let removed = cache.pop_front();
            log::trace!("哈希缓存已满，移除最旧记录: {:?}", removed);
        }

        cache.push_back(hash);
        log::trace!("哈希缓存大小: {}/{}", cache.len(), HASH_CACHE_SIZE);
    }

    /// 获取当前 UUID 缓存大小（用于测试）
    #[allow(dead_code)]
    pub async fn uuid_cache_size(&self) -> usize {
        let cache = self.uuid_cache.lock().await;
        cache.len()
    }

    /// 获取当前哈希缓存大小（用于测试）
    #[allow(dead_code)]
    pub async fn hash_cache_size(&self) -> usize {
        let cache = self.hash_cache.lock().await;
        cache.len()
    }
}

impl Default for DeduplicationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_should_process_message_new() {
        let service = DeduplicationService::new();

        // 新消息应该被处理
        let result = service
            .should_process_message("uuid-1", "hash-1")
            .await;
        assert!(result, "新消息应该被处理");
    }

    #[tokio::test]
    async fn test_should_process_message_duplicate_uuid() {
        let service = DeduplicationService::new();

        // 第一次处理消息
        assert!(service.should_process_message("uuid-1", "hash-1").await);
        service.mark_message_processed("uuid-1".to_string(), "hash-1".to_string()).await;

        // 第二次接收相同 UUID 的消息（UUID 去重生效）
        let result = service.should_process_message("uuid-1", "hash-2").await;
        assert!(!result, "相同 UUID 的消息应该被跳过");
    }

    #[tokio::test]
    async fn test_should_process_message_duplicate_hash() {
        let service = DeduplicationService::new();

        // 第一次处理消息
        assert!(service.should_process_message("uuid-1", "hash-1").await);
        service.mark_message_processed("uuid-1".to_string(), "hash-1".to_string()).await;

        // 第二次接收不同 UUID 但相同哈希的消息（内容哈希去重生效）
        let result = service.should_process_message("uuid-2", "hash-1").await;
        assert!(!result, "相同内容哈希的消息应该被跳过");
    }

    #[tokio::test]
    async fn test_should_send_message_new() {
        let service = DeduplicationService::new();

        // 新内容应该被发送
        let result = service.should_send_message("hash-1").await;
        assert!(result, "新内容应该被发送");
    }

    #[tokio::test]
    async fn test_should_send_message_duplicate() {
        let service = DeduplicationService::new();

        // 第一次发送
        assert!(service.should_send_message("hash-1").await);
        service.mark_message_sent("hash-1".to_string()).await;

        // 第二次发送相同内容
        let result = service.should_send_message("hash-1").await;
        assert!(!result, "相同内容不应该被重复发送");
    }

    #[tokio::test]
    async fn test_uuid_cache_capacity_limit() {
        let service = DeduplicationService::new();

        // 添加超过容量的 UUID
        for i in 0..UUID_CACHE_SIZE + 100 {
            let uuid = format!("uuid-{}", i);
            let hash = format!("hash-{}", i);
            service.mark_message_processed(uuid, hash).await;
        }

        // 确保缓存不超过上限
        let cache_size = service.uuid_cache_size().await;
        assert_eq!(
            cache_size, UUID_CACHE_SIZE,
            "UUID 缓存大小应该保持在上限"
        );

        // 确保最新的 UUID 在缓存中（应该被处理过）
        let latest_uuid = format!("uuid-{}", UUID_CACHE_SIZE + 99);
        let result = service.should_process_message(&latest_uuid, "hash-new").await;
        assert!(!result, "最新的 UUID 应该在缓存中");

        // 确保最旧的 UUID 已被淘汰（可以再次处理）
        let oldest_uuid = "uuid-0";
        let result = service.should_process_message(oldest_uuid, "hash-new-2").await;
        assert!(result, "最旧的 UUID 应该已被淘汰");
    }

    #[tokio::test]
    async fn test_hash_cache_capacity_limit() {
        let service = DeduplicationService::new();

        // 添加超过容量的哈希
        for i in 0..HASH_CACHE_SIZE + 100 {
            let hash = format!("hash-{}", i);
            service.mark_message_sent(hash).await;
        }

        // 确保缓存不超过上限
        let cache_size = service.hash_cache_size().await;
        assert_eq!(
            cache_size, HASH_CACHE_SIZE,
            "哈希缓存大小应该保持在上限"
        );

        // 确保最新的哈希在缓存中
        let latest_hash = format!("hash-{}", HASH_CACHE_SIZE + 99);
        let result = service.should_send_message(&latest_hash).await;
        assert!(!result, "最新的哈希应该在缓存中");

        // 确保最旧的哈希已被淘汰
        let oldest_hash = "hash-0";
        let result = service.should_send_message(oldest_hash).await;
        assert!(result, "最旧的哈希应该已被淘汰");
    }

    #[tokio::test]
    async fn test_double_protection_uuid_failure() {
        // 测试场景：UUID 去重失效时，内容哈希兜底
        let service = DeduplicationService::new();

        // 第一次处理消息
        assert!(service.should_process_message("uuid-1", "hash-1").await);
        service.mark_message_processed("uuid-1".to_string(), "hash-1".to_string()).await;

        // 模拟 UUID 去重失效：相同内容但不同 UUID
        // 内容哈希应该检测到重复
        let result = service.should_process_message("uuid-2", "hash-1").await;
        assert!(!result, "内容哈希应该检测到重复（UUID 去重失效时的兜底）");
    }

    #[tokio::test]
    async fn test_double_protection_hash_collision() {
        // 测试场景：内容哈希碰撞时，UUID 兜底
        let service = DeduplicationService::new();

        // 第一次处理消息
        assert!(service.should_process_message("uuid-1", "hash-collision").await);
        service.mark_message_processed("uuid-1".to_string(), "hash-collision".to_string()).await;

        // 模拟哈希碰撞：不同内容但相同哈希，UUID 不同
        // 在实际场景中，相同哈希表示相同内容，但这里测试的是极端情况
        // UUID 应该检测到相同消息
        let result = service.should_process_message("uuid-1", "hash-different").await;
        assert!(!result, "UUID 去重应该检测到重复（哈希碰撞时的兜底）");
    }

    #[tokio::test]
    async fn test_message_loop_prevention() {
        // 测试场景：防止消息回环（设备 A → 设备 B → 设备 A）
        let service = DeduplicationService::new();

        // 设备 A 发送消息到设备 B
        let uuid = "uuid-loop-test";
        let hash = "hash-loop-test";

        // 设备 B 接收并处理消息
        assert!(service.should_process_message(uuid, hash).await);
        service.mark_message_processed(uuid.to_string(), hash.to_string()).await;

        // 设备 B 的监听器检测到粘贴板变化（来自网络写入）
        // 应该跳过推送（通过内容哈希去重）
        let result = service.should_send_message(hash).await;
        assert!(!result, "来自网络的内容不应该被再次推送");

        // 即使消息回环回到设备 B，也应该被 UUID 去重拦截
        let result = service.should_process_message(uuid, hash).await;
        assert!(!result, "回环的消息应该被 UUID 去重拦截");
    }

    #[tokio::test]
    async fn test_clone_service() {
        let service = DeduplicationService::new();

        // 标记消息已处理
        service.mark_message_processed("uuid-1".to_string(), "hash-1".to_string()).await;

        // 克隆服务（共享相同的缓存）
        let cloned_service = service.clone();

        // 克隆的服务应该能看到相同的缓存
        let result = cloned_service.should_process_message("uuid-1", "hash-2").await;
        assert!(!result, "克隆的服务应该共享相同的 UUID 缓存");

        let result = cloned_service.should_send_message("hash-1").await;
        assert!(!result, "克隆的服务应该共享相同的哈希缓存");
    }
}

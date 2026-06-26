// 消息去重机制集成测试
//
// 测试范围：
// - UUID 去重集成测试
// - 内容哈希去重集成测试
// - 双重保险场景测试
// - 缓存容量管理测试
// - 消息回环阻止测试

use paseboard::clipboard::DeduplicationService;
use sha2::{Digest, Sha256};

/// 计算内容的 SHA256 哈希（与 ClipboardMonitor 中的实现一致）
fn calculate_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// 生成模拟的消息 UUID
fn generate_uuid(index: usize) -> String {
    format!("uuid-{:08x}", index)
}

#[tokio::test]
async fn test_uuid_deduplication_integration() {
    // 测试 UUID 去重在消息接收流程中的集成

    let service = DeduplicationService::new();

    // 场景 1：接收新消息
    let uuid1 = generate_uuid(1);
    let content1 = "Hello, World!";
    let hash1 = calculate_hash(content1);

    assert!(
        service.should_process_message(&uuid1, &hash1).await,
        "新消息应该被处理"
    );

    // 标记消息已处理
    service.mark_message_processed(uuid1.clone(), hash1.clone()).await;

    // 场景 2：接收重复 UUID 的消息（应该被拦截）
    assert!(
        !service.should_process_message(&uuid1, &hash1).await,
        "重复 UUID 的消息应该被跳过"
    );

    // 场景 3：不同 UUID 的新消息
    let uuid2 = generate_uuid(2);
    let content2 = "Different content";
    let hash2 = calculate_hash(content2);

    assert!(
        service.should_process_message(&uuid2, &hash2).await,
        "不同 UUID 的新消息应该被处理"
    );
}

#[tokio::test]
async fn test_content_hash_deduplication_integration() {
    // 测试内容哈希去重在监听流程中的集成

    let service = DeduplicationService::new();

    // 场景 1：监听到新内容
    let content1 = "First clipboard content";
    let hash1 = calculate_hash(content1);

    assert!(
        service.should_send_message(&hash1).await,
        "新内容应该被发送"
    );

    // 标记消息已发送
    service.mark_message_sent(hash1.clone()).await;

    // 场景 2：监听到相同内容（应该被跳过）
    assert!(
        !service.should_send_message(&hash1).await,
        "相同内容不应该被重复发送"
    );

    // 场景 3：监听到不同内容
    let content2 = "Second clipboard content";
    let hash2 = calculate_hash(content2);

    assert!(
        service.should_send_message(&hash2).await,
        "不同内容应该被发送"
    );
}

#[tokio::test]
async fn test_double_protection_scenario() {
    // 测试双重保险机制

    let service = DeduplicationService::new();

    // 场景 1：UUID 去重失效时，内容哈希兜底
    let uuid1 = generate_uuid(1);
    let content = "Shared content";
    let hash = calculate_hash(content);

    // 第一次处理消息
    assert!(service.should_process_message(&uuid1, &hash).await);
    service.mark_message_processed(uuid1.clone(), hash.clone()).await;

    // 模拟 UUID 去重失效：不同 UUID 但相同内容
    let uuid2 = generate_uuid(2);
    assert!(
        !service.should_process_message(&uuid2, &hash).await,
        "内容哈希应该检测到重复（UUID 去重失效时的兜底）"
    );

    // 场景 2：内容哈希碰撞时，UUID 兜底
    let uuid3 = generate_uuid(3);
    let different_content = "Different content";
    let different_hash = calculate_hash(different_content);

    // 处理新消息
    assert!(service.should_process_message(&uuid3, &different_hash).await);
    service.mark_message_processed(uuid3.clone(), different_hash.clone()).await;

    // 相同 UUID 再次出现（即使哈希不同，也应该被拦截）
    let another_hash = calculate_hash("Yet another content");
    assert!(
        !service.should_process_message(&uuid3, &another_hash).await,
        "UUID 去重应该检测到重复（哈希碰撞时的兜底）"
    );
}

#[tokio::test]
async fn test_cache_capacity_management() {
    // 测试缓存容量管理（1000 条上限）

    let service = DeduplicationService::new();

    // 场景 1：缓存未满时直接添加
    for i in 0..500 {
        let uuid = generate_uuid(i);
        let content = format!("Content {}", i);
        let hash = calculate_hash(&content);

        assert!(service.should_process_message(&uuid, &hash).await);
        service.mark_message_processed(uuid, hash).await;
    }

    assert_eq!(
        service.uuid_cache_size().await,
        500,
        "缓存应该有 500 条记录"
    );

    // 场景 2：缓存已满时淘汰最旧记录
    for i in 500..1100 {
        let uuid = generate_uuid(i);
        let content = format!("Content {}", i);
        let hash = calculate_hash(&content);

        service.mark_message_processed(uuid, hash).await;
    }

    assert_eq!(
        service.uuid_cache_size().await,
        1000,
        "缓存应该保持在 1000 条上限"
    );

    // 验证最旧的记录已被淘汰
    let oldest_uuid = generate_uuid(0);
    let oldest_content = "Content 0";
    let oldest_hash = calculate_hash(oldest_content);

    assert!(
        service.should_process_message(&oldest_uuid, &oldest_hash).await,
        "最旧的记录应该已被淘汰，可以再次处理"
    );

    // 验证最新的记录还在缓存中
    let newest_uuid = generate_uuid(1099);
    let newest_content = "Content 1099";
    let newest_hash = calculate_hash(&newest_content);

    assert!(
        !service.should_process_message(&newest_uuid, &newest_hash).await,
        "最新的记录应该还在缓存中"
    );
}

#[tokio::test]
async fn test_message_loop_prevention() {
    // 测试消息回环阻止（设备 A → 设备 B → 设备 A）

    let service = DeduplicationService::new();

    // 场景：设备 A 发送消息到设备 B
    let uuid = generate_uuid(1);
    let content = "Message from Device A";
    let hash = calculate_hash(content);

    // 步骤 1：设备 B 接收消息
    assert!(
        service.should_process_message(&uuid, &hash).await,
        "设备 B 应该处理来自设备 A 的消息"
    );
    service.mark_message_processed(uuid.clone(), hash.clone()).await;

    // 步骤 2：设备 B 写入粘贴板后，监听器检测到内容变化
    // 此时应该跳过推送（因为内容哈希已在缓存中）
    assert!(
        !service.should_send_message(&hash).await,
        "设备 B 不应该将来自网络的内容再次推送"
    );

    // 步骤 3：即使消息回环回到设备 B，也应该被拦截
    assert!(
        !service.should_process_message(&uuid, &hash).await,
        "回环的消息应该被 UUID 去重拦截"
    );

    // 验证新内容仍能正常传播
    let new_content = "New content from Device B user";
    let new_hash = calculate_hash(new_content);

    assert!(
        service.should_send_message(&new_hash).await,
        "设备 B 用户复制的新内容应该能正常推送"
    );
}

#[tokio::test]
async fn test_rapid_duplicate_messages() {
    // 测试快速连续重复消息的处理

    let service = DeduplicationService::new();

    let uuid = generate_uuid(1);
    let content = "Rapid message";
    let hash = calculate_hash(content);

    // 第一条消息应该被处理
    assert!(service.should_process_message(&uuid, &hash).await);
    service.mark_message_processed(uuid.clone(), hash.clone()).await;

    // 快速连续的重复消息应该全部被拦截
    for _ in 0..10 {
        assert!(
            !service.should_process_message(&uuid, &hash).await,
            "快速重复的消息应该被拦截"
        );
    }
}

#[tokio::test]
async fn test_concurrent_deduplication() {
    // 测试并发场景下的去重

    let service = DeduplicationService::new();

    // 模拟多个任务并发处理消息
    let mut handles = vec![];

    for i in 0..10 {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            let uuid = generate_uuid(i);
            let content = format!("Concurrent message {}", i);
            let hash = calculate_hash(&content);

            // 每个消息都应该被处理一次
            if service_clone.should_process_message(&uuid, &hash).await {
                service_clone.mark_message_processed(uuid, hash).await;
                true
            } else {
                false
            }
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // 所有消息都应该被处理（因为它们各不相同）
    assert_eq!(
        results.iter().filter(|&&r| r).count(),
        10,
        "所有不同的消息都应该被处理"
    );

    assert_eq!(
        service.uuid_cache_size().await,
        10,
        "缓存应该有 10 条记录"
    );
}

#[tokio::test]
async fn test_hash_collision_edge_case() {
    // 测试哈希碰撞的边缘情况

    let service = DeduplicationService::new();

    // 在真实场景中，SHA256 碰撞的概率极低
    // 这里模拟一个极端情况：两个消息有相同的哈希但不同的 UUID

    let uuid1 = generate_uuid(1);
    let uuid2 = generate_uuid(2);
    let same_hash = "collision-hash-simulation";

    // 第一条消息
    assert!(service.should_process_message(&uuid1, same_hash).await);
    service.mark_message_processed(uuid1.clone(), same_hash.to_string()).await;

    // 第二条消息（相同哈希，不同 UUID）
    // 应该被内容哈希去重拦截
    assert!(
        !service.should_process_message(&uuid2, same_hash).await,
        "相同哈希的消息应该被拦截（即使 UUID 不同）"
    );

    // 但如果第一条消息的 UUID 再次出现（不同哈希）
    // 应该被 UUID 去重拦截
    let different_hash = "different-hash";
    assert!(
        !service.should_process_message(&uuid1, different_hash).await,
        "相同 UUID 的消息应该被拦截（即使哈希不同）"
    );
}

#[tokio::test]
async fn test_send_and_receive_integration() {
    // 测试发送和接收流程的完整集成
    // 注意：发送端和接收端应该使用不同的去重服务实例（模拟不同设备）

    let sender_service = DeduplicationService::new();
    let receiver_service = DeduplicationService::new();

    // 场景：设备 A 复制内容并发送
    let content = "Integration test content";
    let hash = calculate_hash(content);

    // 步骤 1：设备 A 监听器检测到内容变化
    assert!(
        sender_service.should_send_message(&hash).await,
        "新内容应该被发送"
    );
    sender_service.mark_message_sent(hash.clone()).await;

    // 步骤 2：生成消息并发送
    let uuid = generate_uuid(1);

    // 步骤 3：设备 B 接收消息
    assert!(
        receiver_service.should_process_message(&uuid, &hash).await,
        "设备 B 应该处理消息"
    );
    receiver_service.mark_message_processed(uuid.clone(), hash.clone()).await;

    // 步骤 4：设备 B 写入粘贴板后，监听器检测到变化
    // 应该跳过推送（内容哈希已在缓存中）
    assert!(
        !receiver_service.should_send_message(&hash).await,
        "设备 B 不应该再次推送相同内容"
    );

    // 步骤 5：设备 B 也不应该再次处理相同的消息
    assert!(
        !receiver_service.should_process_message(&uuid, &hash).await,
        "设备 B 不应该重复处理相同的消息"
    );

    // 步骤 6：设备 A 也不应该再次发送相同内容
    assert!(
        !sender_service.should_send_message(&hash).await,
        "设备 A 不应该重复发送相同内容"
    );
}

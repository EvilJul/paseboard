// 历史记录存储模块
//
// 职责：
// - SQLite 数据库初始化和管理
// - 历史记录插入、查询
// - 容量管理（最多 1000 条，超过则删除最旧的 100 条）
// - 索引优化（timestamp DESC, content_hash）

use crate::utils::error::StorageError;
use rusqlite::{Connection, params};
use sha2::{Sha256, Digest};
use std::path::Path;

/// 历史记录存储管理器
pub struct HistoryStorage {
    conn: Connection,
}

/// 历史记录项
#[derive(Debug, Clone)]
pub struct HistoryItem {
    pub id: i64,
    pub content: String,
    pub content_hash: String,
    pub device_id: String,
    pub device_name: String,
    pub timestamp: i64,
    pub size: i64,
}

impl HistoryStorage {
    /// 创建新的历史存储管理器
    ///
    /// # Arguments
    /// * `db_path` - 数据库文件路径（如 ~/.paseboard/history.db）
    ///
    /// # Returns
    /// 初始化后的存储管理器
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, StorageError> {
        // 确保数据库目录存在
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        }

        // 打开数据库连接
        let conn = Connection::open(db_path)?;

        // 初始化数据库表和索引
        Self::init_database(&conn)?;

        Ok(Self { conn })
    }

    /// 初始化数据库表结构和索引
    fn init_database(conn: &Connection) -> Result<(), StorageError> {
        // 创建历史记录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                size INTEGER NOT NULL
            )",
            [],
        )?;

        // 创建时间戳索引（降序，优化查询最近记录）
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp
             ON clipboard_history(timestamp DESC)",
            [],
        )?;

        // 创建内容哈希唯一索引（用于去重 + 防并发重复）
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_content_hash
             ON clipboard_history(content_hash)",
            [],
        )?;

        Ok(())
    }

    /// 插入新的历史记录
    ///
    /// # Arguments
    /// * `content` - 粘贴板内容
    /// * `device_id` - 设备 UUID
    /// * `device_name` - 设备名称
    ///
    /// # Returns
    /// 插入的记录 ID（跳过时返回 -1）
    pub fn insert(
        &mut self,
        content: &str,
        device_id: &str,
        device_name: &str,
    ) -> Result<i64, StorageError> {
        // 跳过空内容（仅空白字符）
        if content.trim().is_empty() {
            return Ok(-1);
        }

        // 计算内容哈希
        let content_hash = Self::compute_hash(content);

        // 查询最近一条记录的哈希，相同内容则跳过
        let last_hash: Option<String> = self
            .conn
            .query_row(
                "SELECT content_hash FROM clipboard_history ORDER BY timestamp DESC, id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(ref hash) = last_hash {
            if hash == &content_hash {
                return Ok(-1);
            }
        }

        // 计算内容大小（字节）
        let size = content.len() as i64;

        // 获取当前时间戳（Unix 时间戳，秒）
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // 检查并执行容量管理
        self.enforce_capacity_limit()?;

        // 插入记录（唯一索引自动防并发重复）
        self.conn.execute(
            "INSERT OR IGNORE INTO clipboard_history
             (content, content_hash, device_id, device_name, timestamp, size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![content, content_hash, device_id, device_name, timestamp, size],
        )?;

        if self.conn.changes() == 0 {
            return Ok(-1);
        }

        Ok(self.conn.last_insert_rowid())
    }

    /// 容量管理：如果记录数达到 1000 条，删除最旧的 100 条
    fn enforce_capacity_limit(&mut self) -> Result<(), StorageError> {
        // 查询当前记录总数
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history",
            [],
            |row| row.get(0),
        )?;

        // 如果达到容量上限，删除最旧的 100 条
        if count >= 1000 {
            self.conn.execute(
                "DELETE FROM clipboard_history
                 WHERE id IN (
                     SELECT id FROM clipboard_history
                     ORDER BY timestamp ASC, id ASC
                     LIMIT 100
                 )",
                [],
            )?;
        }

        Ok(())
    }

    /// 查询最近的 N 条历史记录（按时间倒序）
    ///
    /// # Arguments
    /// * `limit` - 查询条数限制
    ///
    /// # Returns
    /// 历史记录列表（最新的在前）
    pub fn query_recent(&self, limit: usize) -> Result<Vec<HistoryItem>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, content_hash, device_id, device_name, timestamp, size
             FROM clipboard_history
             ORDER BY timestamp DESC, id DESC
             LIMIT ?1",
        )?;

        let items = stmt.query_map([limit], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                content: row.get(1)?,
                content_hash: row.get(2)?,
                device_id: row.get(3)?,
                device_name: row.get(4)?,
                timestamp: row.get(5)?,
                size: row.get(6)?,
            })
        })?;

        let mut result = Vec::new();
        let mut last_content: Option<String> = None;
        for item in items {
            let item = item?;
            // 去重去空：跳过内容为空或与前一条内容相同的记录
            if item.content.trim().is_empty() {
                continue;
            }
            if let Some(ref last) = last_content {
                if last == &item.content {
                    continue;
                }
            }
            last_content = Some(item.content.clone());
            result.push(item);
        }

        Ok(result)
    }

    /// 根据内容哈希检查记录是否已存在
    ///
    /// # Arguments
    /// * `content_hash` - 内容哈希值
    ///
    /// # Returns
    /// 是否存在
    pub fn exists_by_hash(&self, content_hash: &str) -> Result<bool, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history WHERE content_hash = ?1",
            [content_hash],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// 计算内容的 SHA256 哈希
    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 获取历史记录总数
    pub fn count(&self) -> Result<i64, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_history",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 清空所有历史记录（用于测试）
    #[cfg(test)]
    pub fn clear(&mut self) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM clipboard_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建临时数据库用于测试
    fn create_temp_storage() -> HistoryStorage {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_history_{}.db", uuid::Uuid::new_v4()));
        HistoryStorage::new(&db_path).unwrap()
    }

    #[test]
    fn test_database_initialization() {
        let storage = create_temp_storage();

        // 验证表是否创建
        let table_exists: i64 = storage.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='clipboard_history'",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(table_exists, 1);

        // 验证索引是否创建
        let index_count: i64 = storage.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_timestamp', 'idx_content_hash')",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(index_count, 2);
    }

    #[test]
    fn test_insert_record() {
        let mut storage = create_temp_storage();

        // 插入一条记录
        let id = storage.insert(
            "测试内容",
            "device-123",
            "测试设备",
        ).unwrap();

        assert!(id > 0);

        // 验证记录数
        let count = storage.count().unwrap();
        assert_eq!(count, 1);

        // 查询记录
        let items = storage.query_recent(10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "测试内容");
        assert_eq!(items[0].device_id, "device-123");
        assert_eq!(items[0].device_name, "测试设备");
    }

    #[test]
    fn test_capacity_management() {
        let mut storage = create_temp_storage();

        // 插入 1005 条记录
        for i in 0..1005 {
            storage.insert(
                &format!("内容 {}", i),
                "device-test",
                "测试设备",
            ).unwrap();
        }

        // 验证记录数应该是 905（1005 - 100）
        let count = storage.count().unwrap();
        assert_eq!(count, 905);

        // 验证最旧的 100 条已被删除
        let items = storage.query_recent(1000).unwrap();
        assert_eq!(items.len(), 905);

        // 最新一条应该是 "内容 1004"（最后插入的）
        assert_eq!(items[0].content, "内容 1004");

        // 最旧一条应该是 "内容 100"（前 100 条被删除）
        let oldest = items.last().unwrap();
        assert_eq!(oldest.content, "内容 100");
    }

    #[test]
    fn test_query_with_limit() {
        let mut storage = create_temp_storage();

        // 插入 50 条记录
        for i in 0..50 {
            storage.insert(
                &format!("内容 {}", i),
                "device-test",
                "测试设备",
            ).unwrap();
        }

        // 查询最近 10 条
        let items = storage.query_recent(10).unwrap();
        assert_eq!(items.len(), 10);

        // 验证顺序（最新的在前）
        assert_eq!(items[0].content, "内容 49");
        assert_eq!(items[9].content, "内容 40");
    }

    #[test]
    fn test_content_hash() {
        let mut storage = create_temp_storage();

        // 插入一条记录
        storage.insert("Hello, World!", "device-1", "设备1").unwrap();

        // 查询记录
        let items = storage.query_recent(1).unwrap();
        let hash = &items[0].content_hash;

        // 验证哈希不为空
        assert!(!hash.is_empty());

        // 验证相同内容产生相同哈希
        let hash2 = HistoryStorage::compute_hash("Hello, World!");
        assert_eq!(hash, &hash2);

        // 验证不同内容产生不同哈希
        let hash3 = HistoryStorage::compute_hash("Different content");
        assert_ne!(hash, &hash3);
    }

    #[test]
    fn test_exists_by_hash() {
        let mut storage = create_temp_storage();

        // 插入一条记录
        storage.insert("测试内容", "device-1", "设备1").unwrap();

        // 计算哈希
        let hash = HistoryStorage::compute_hash("测试内容");

        // 验证存在
        assert!(storage.exists_by_hash(&hash).unwrap());

        // 验证不存在的哈希
        let non_exist_hash = HistoryStorage::compute_hash("不存在的内容");
        assert!(!storage.exists_by_hash(&non_exist_hash).unwrap());
    }

    #[test]
    fn test_timestamp_ordering() {
        let mut storage = create_temp_storage();

        // 插入 3 条记录，中间有延迟确保时间戳不同
        storage.insert("第一条", "device-1", "设备1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));

        storage.insert("第二条", "device-1", "设备1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));

        storage.insert("第三条", "device-1", "设备1").unwrap();

        // 查询所有记录
        let items = storage.query_recent(10).unwrap();

        // 验证时间戳降序排列
        assert!(items[0].timestamp >= items[1].timestamp);
        assert!(items[1].timestamp >= items[2].timestamp);

        // 验证内容顺序（最新的在前）
        assert_eq!(items[0].content, "第三条");
        assert_eq!(items[1].content, "第二条");
        assert_eq!(items[2].content, "第一条");
    }

    #[test]
    fn test_unique_constraint() {
        let mut storage = create_temp_storage();

        // 插入一条记录
        let id1 = storage.insert("相同内容", "device-1", "设备1").unwrap();
        assert!(id1 > 0);

        // 再次插入相同内容 - UNIQUE 索引应阻止
        let id2 = storage.insert("相同内容", "device-1", "设备1").unwrap();
        assert_eq!(id2, -1);

        // 验证 DB 中只有 1 条
        assert_eq!(storage.count().unwrap(), 1);

        // 插入不同内容应正常
        let id3 = storage.insert("不同内容", "device-1", "设备1").unwrap();
        assert!(id3 > 0);
        assert_eq!(storage.count().unwrap(), 2);
    }

    #[test]
    fn test_unique_constraint_different_devices() {
        let mut storage = create_temp_storage();

        // 同一内容来自不同设备——同样应被 UNIQUE 索引阻止
        storage.insert("相同", "device-1", "设备1").unwrap();
        let id = storage.insert("相同", "device-2", "设备2").unwrap();
        assert_eq!(id, -1);
        assert_eq!(storage.count().unwrap(), 1);
    }

    #[test]
    fn test_empty_content_skipped() {
        let mut storage = create_temp_storage();

        // 空白内容应被跳过（不进入 DB）
        let id = storage.insert("   ", "device-1", "设备1").unwrap();
        assert_eq!(id, -1);
        assert_eq!(storage.count().unwrap(), 0);

        // 空字符串也应被跳过
        let id = storage.insert("", "device-1", "设备1").unwrap();
        assert_eq!(id, -1);
        assert_eq!(storage.count().unwrap(), 0);
    }

    #[test]
    fn test_query_recent_dedup() {
        let mut storage = create_temp_storage();

        // 插入 A → B → B → C，期望查询返回 A → B → C
        storage.insert("A", "device-1", "设备1").unwrap();
        storage.insert("B", "device-1", "设备1").unwrap();
        storage.insert("B", "device-1", "设备1").unwrap();
        storage.insert("C", "device-1", "设备1").unwrap();

        let items = storage.query_recent(10).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].content, "C");
        assert_eq!(items[1].content, "B");
        assert_eq!(items[2].content, "A");
    }

    #[test]
    fn test_size_calculation() {
        let mut storage = create_temp_storage();

        // 插入不同大小的内容
        storage.insert("a", "device-1", "设备1").unwrap();
        storage.insert("Hello", "device-1", "设备1").unwrap();
        storage.insert("你好世界", "device-1", "设备1").unwrap();

        // 查询记录
        let items = storage.query_recent(10).unwrap();

        // 验证大小（按字节计算，最新的在前）
        assert_eq!(items[0].size, 12); // "你好世界" = 12 字节（UTF-8 编码）
        assert_eq!(items[1].size, 5);  // "Hello" = 5 字节
        assert_eq!(items[2].size, 1);  // "a" = 1 字节
    }
}

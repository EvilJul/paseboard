// 配置管理模块
//
// 负责加载和管理应用配置，包括设备 UUID、设备名称等

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// 应用配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 设备唯一标识符（UUID v4）
    pub device_id: String,

    /// 设备名称（默认使用计算机名称）
    pub device_name: String,

    /// 自定义设备名称（用户自定义，优先级高于 device_name）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_device_name: Option<String>,

    /// WebSocket 监听端口（默认 9527，支持自动降级到 9528-9537）
    pub port: u16,

    /// 配置文件路径
    #[serde(skip)]
    pub config_path: PathBuf,
}

impl AppConfig {
    /// 从配置文件加载配置，如果文件不存在则创建默认配置
    pub fn load() -> anyhow::Result<Self> {
        let config_dir = Self::config_dir()?;
        let config_path = config_dir.join("config.toml");

        // 如果配置文件存在，尝试加载
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut config: AppConfig = toml::from_str(&content)?;
            config.config_path = config_path;
            Ok(config)
        } else {
            // 创建默认配置
            let config = Self::create_default(config_path)?;
            Ok(config)
        }
    }

    /// 创建默认配置并保存到文件
    fn create_default(config_path: PathBuf) -> anyhow::Result<Self> {
        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 生成设备 UUID
        let device_id = Uuid::new_v4().to_string();

        // 获取计算机名称作为设备名称（去掉 .local 后缀避免重复）
        let device_name = hostname::get()
            .ok()
            .and_then(|name| name.into_string().ok())
            .map(|name| name.trim_end_matches(".local").to_string())
            .unwrap_or_else(|| "Unknown Device".to_string());

        let config = AppConfig {
            device_id,
            device_name,
            custom_device_name: None,
            port: 9527,
            config_path: config_path.clone(),
        };

        // 保存配置到文件
        config.save()?;

        Ok(config)
    }

    /// 保存配置到文件
    pub fn save(&self) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// 获取配置目录路径（~/.paseboard/）
    fn config_dir() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取用户主目录"))?;
        Ok(home.join(".paseboard"))
    }

    /// 获取历史数据库文件路径
    pub fn db_path(&self) -> anyhow::Result<PathBuf> {
        Ok(Self::config_dir()?.join("history.db"))
    }

    /// 获取设备身份密钥文件路径
    pub fn identity_path(&self) -> PathBuf {
        Self::config_dir().unwrap().join("identity.pem")
    }

    /// 获取显示用的设备名称（优先使用自定义名称）
    pub fn display_name(&self) -> &str {
        self.custom_device_name
            .as_deref()
            .unwrap_or(&self.device_name)
    }

    /// 更新自定义设备名称
    pub fn set_custom_device_name(&mut self, name: Option<String>) -> anyhow::Result<()> {
        self.custom_device_name = name;
        self.save()
    }

    /// 验证设备名称是否有效
    pub fn validate_device_name(name: &str) -> Result<(), String> {
        // 检查是否为空
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("名称不能为空".to_string());
        }

        // 检查长度
        if trimmed.len() > 30 {
            return Err("名称长度不能超过 30 个字符".to_string());
        }

        // 检查字符：允许中文、英文、数字、空格、连字符
        let is_valid = trimmed.chars().all(|c| {
            c.is_alphanumeric()  // 字母和数字
                || c == ' '      // 空格
                || c == '-'      // 连字符
                || (c >= '\u{4E00}' && c <= '\u{9FFF}')  // CJK 统一汉字
                || (c >= '\u{3400}' && c <= '\u{4DBF}')  // CJK 扩展 A
        });

        if !is_valid {
            return Err("名称只能包含中文、英文、数字、空格和连字符".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir() {
        let config_dir = AppConfig::config_dir().unwrap();
        assert!(config_dir.to_string_lossy().contains(".paseboard"));
    }
}

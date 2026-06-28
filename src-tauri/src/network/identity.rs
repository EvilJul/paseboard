// 设备身份模块
//
// 职责：
// - Ed25519 密钥对生成与持久化（PKCS#8 PEM）
// - 设备 ID 派生（sha256(public_key)[:16] hex）
// - 签名和验签

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use ed25519_dalek::pkcs8::{EncodePrivateKey, DecodePrivateKey};
use rand::RngCore;
use sha2::{Sha256, Digest};
use thiserror::Error;

/// 设备身份错误
#[derive(Debug, Error)]
pub enum IdentityError {
    /// 密钥文件读写失败
    #[error("密钥文件读写失败: {0}")]
    IoError(#[from] std::io::Error),

    /// 密钥解析失败（文件格式错误或密钥损坏）
    #[error("密钥解析失败: {0}")]
    KeyParseFailed(String),

    /// 密钥序列化失败
    #[error("密钥序列化失败: {0}")]
    KeySerializationFailed(String),

    /// 签名操作失败
    #[error("签名失败: {0}")]
    SignFailed(String),

    /// 签名验证失败
    #[error("签名验证失败: {0}")]
    VerificationFailed(String),
}

/// 设备身份管理器
///
/// 管理 Ed25519 密钥对的生成、持久化、设备 ID 派生、签名和验签。
pub struct IdentityManager {
    /// Ed25519 签名密钥
    signing_key: SigningKey,
    /// Ed25519 验证密钥
    verifying_key: VerifyingKey,
    /// 32 字节公钥缓存
    public_key: [u8; 32],
    /// 设备 ID（sha256(public_key)[:16] 的 hex 字符串，32 字符）
    device_id: String,
    /// 密钥文件路径
    key_path: PathBuf,
}

/// 将 PKCS#8 DER 编码转为 PEM 文本（RFC 7468）
fn der_to_pem(der: &[u8]) -> String {
    let b64 = BASE64.encode(der);
    let mut pem = String::from("-----BEGIN PRIVATE KEY-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap());
        pem.push('\n');
    }
    pem.push_str("-----END PRIVATE KEY-----\n");
    pem
}

/// 将 PEM 文本解析为 PKCS#8 DER 编码
fn pem_to_der(pem: &str) -> Result<Vec<u8>, IdentityError> {
    let mut in_body = false;
    let mut b64 = String::new();
    for line in pem.lines() {
        let trimmed = line.trim();
        if trimmed == "-----BEGIN PRIVATE KEY-----" {
            in_body = true;
            continue;
        }
        if trimmed == "-----END PRIVATE KEY-----" {
            break;
        }
        if in_body && !trimmed.is_empty() {
            b64.push_str(trimmed);
        }
    }
    BASE64
        .decode(&b64)
        .map_err(|e| IdentityError::KeyParseFailed(format!("Base64 解码失败: {}", e)))
}

impl IdentityManager {
    /// 创建或加载设备身份
    ///
    /// 如果 `key_path` 文件已存在，从 PKCS#8 PEM 文件加载密钥对；
    /// 否则生成新的 Ed25519 密钥对并持久化到 `key_path`。
    ///
    /// # 参数
    /// - `key_path`: 密钥文件路径（通常为 `~/.paseboard/identity.pem`）
    ///
    /// # 返回
    /// - `Ok(IdentityManager)`: 成功创建身份管理器
    /// - `Err(IdentityError)`: 密钥生成或加载失败
    pub fn new(key_path: PathBuf) -> Result<Self, IdentityError> {
        let (signing_key, verifying_key) = if key_path.exists() {
            let pem = std::fs::read_to_string(&key_path)?;
            let der = pem_to_der(&pem)?;
            let signing_key = SigningKey::from_pkcs8_der(&der)
                .map_err(|e| IdentityError::KeyParseFailed(e.to_string()))?;
            let verifying_key = signing_key.verifying_key();
            (signing_key, verifying_key)
        } else {
            let mut secret = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut secret);
            let signing_key = SigningKey::from_bytes(&secret);
            let verifying_key = signing_key.verifying_key();

            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let der = signing_key
                .to_pkcs8_der()
                .map_err(|e| IdentityError::KeySerializationFailed(e.to_string()))?;
            let pem = der_to_pem(der.as_bytes());
            std::fs::write(&key_path, pem.as_bytes())?;

            (signing_key, verifying_key)
        };

        let pub_key_bytes = verifying_key.to_bytes();
        let hash = Sha256::digest(&pub_key_bytes);
        let device_id = hash[..16]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        Ok(Self {
            signing_key,
            verifying_key,
            public_key: pub_key_bytes,
            device_id,
            key_path,
        })
    }

    /// 获取设备 ID
    ///
    /// 设备 ID 是公钥 SHA256 的前 16 字节的 hex 字符串，共 32 字符。
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// 获取 Base64 编码的公钥
    ///
    /// 用于 mDNS TXT 记录广播，供其他设备进行身份验证。
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key)
    }

    /// 获取原始公钥字节引用
    pub fn public_key_bytes(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// 对数据进行 Ed25519 签名
    ///
    /// # 参数
    /// - `data`: 待签名的数据
    ///
    /// # 返回
    /// - `Ok(Vec<u8>)`: 64 字节的签名数据
    /// - `Err(IdentityError)`: 签名失败
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, IdentityError> {
        let signature = self.signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    /// 验证 Ed25519 签名
    ///
    /// 使用指定的公钥验证数据签名（不限于本设备公钥，可用于验证任意设备）。
    ///
    /// # 参数
    /// - `data`: 被签名的原始数据
    /// - `signature`: 待验证的 64 字节签名
    /// - `public_key`: 签名者公钥
    ///
    /// # 返回
    /// `true` 签名有效，`false` 签名无效或解析失败
    pub fn verify(&self, data: &[u8], signature: &[u8], public_key: &VerifyingKey) -> bool {
        let sig = match Signature::from_slice(signature) {
            Ok(s) => s,
            Err(_) => return false,
        };
        public_key.verify(data, &sig).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn test_key_path() -> PathBuf {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut path = env::temp_dir();
        path.push(format!("paseboard_test_identity_{}_{}.pem", std::process::id(), count));
        path
    }

    #[test]
    fn test_identity_generation_and_persistence() {
        let path = test_key_path();
        let _ = std::fs::remove_file(&path);

        let manager = IdentityManager::new(path.clone()).unwrap();
        let device_id = manager.device_id().to_string();
        let pub_key_b64 = manager.public_key_base64();
        assert_eq!(device_id.len(), 32);
        assert!(!pub_key_b64.is_empty());
        assert!(path.exists());

        let manager2 = IdentityManager::new(path.clone()).unwrap();
        assert_eq!(manager2.device_id(), device_id);
        assert_eq!(manager2.public_key_base64(), pub_key_b64);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_device_id_derivation() {
        let path = test_key_path();
        let _ = std::fs::remove_file(&path);

        let manager = IdentityManager::new(path.clone()).unwrap();
        let device_id = manager.device_id();

        assert_eq!(device_id.len(), 32);
        assert!(device_id.chars().all(|c| c.is_ascii_hexdigit()));

        let hash = Sha256::digest(manager.public_key_bytes());
        let expected: String = hash[..16]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        assert_eq!(device_id, expected);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sign_and_verify() {
        let path = test_key_path();
        let _ = std::fs::remove_file(&path);

        let manager = IdentityManager::new(path.clone()).unwrap();
        let data = b"Hello, PaseBoard!";

        let signature = manager.sign(data).unwrap();
        assert_eq!(signature.len(), 64);

        let pub_key = VerifyingKey::from_bytes(manager.public_key_bytes()).unwrap();
        assert!(manager.verify(data, &signature, &pub_key));
        assert!(!manager.verify(b"tampered data", &signature, &pub_key));

        let mut bad_sig = signature.clone();
        bad_sig[0] ^= 0xff;
        assert!(!manager.verify(data, &bad_sig, &pub_key));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_public_key_base64_roundtrip() {
        let path = test_key_path();
        let _ = std::fs::remove_file(&path);

        let manager = IdentityManager::new(path.clone()).unwrap();
        let b64 = manager.public_key_base64();

        let decoded = BASE64.decode(&b64).unwrap();
        assert_eq!(decoded.len(), 32);
        assert_eq!(decoded.as_slice(), manager.public_key_bytes());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_pem_roundtrip() {
        let path = test_key_path();
        let _ = std::fs::remove_file(&path);

        let _manager = IdentityManager::new(path.clone()).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(content.trim().ends_with("-----END PRIVATE KEY-----"));

        let der = pem_to_der(&content).unwrap();
        assert!(!der.is_empty());

        let restored = der_to_pem(&der);
        assert_eq!(content.trim(), restored.trim());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_signature_verify_with_wrong_key() {
        let path1 = test_key_path();
        let path2 = {
            let mut p = env::temp_dir();
            p.push(format!("paseboard_test_identity_{}_2.pem", std::process::id()));
            p
        };
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);

        let manager1 = IdentityManager::new(path1.clone()).unwrap();
        let manager2 = IdentityManager::new(path2.clone()).unwrap();

        let data = b"cross-device test";
        let sig = manager1.sign(data).unwrap();

        // manager1 的公钥可以验证
        let pk1 = VerifyingKey::from_bytes(manager1.public_key_bytes()).unwrap();
        assert!(manager1.verify(data, &sig, &pk1));

        // manager2 的公钥不能验证 manager1 的签名
        let pk2 = VerifyingKey::from_bytes(manager2.public_key_bytes()).unwrap();
        assert!(!manager1.verify(data, &sig, &pk2));

        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }
}

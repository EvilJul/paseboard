// WebSocket 加密传输层
//
// 职责：
// - AES-256-GCM 加密/解密
// - ECDH (X25519) 密钥交换
// - HKDF 派生对称密钥
// - 会话管理

use std::sync::Arc;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use thiserror::Error;

use super::identity::IdentityManager;

/// 加密模块错误
#[derive(Debug, Error)]
pub enum CryptoError {
    /// 密钥交换失败
    #[error("密钥交换失败: {0}")]
    KeyExchangeFailed(String),

    /// 加密失败
    #[error("加密失败: {0}")]
    EncryptFailed(String),

    /// 解密失败
    #[error("解密失败: {0}")]
    DecryptFailed(String),

    /// 无效的加密载荷
    #[error("无效的加密载荷")]
    InvalidPayload,
}

/// 加密载荷，包含解密所需的所有信息
pub struct EncryptedPayload {
    /// 12 字节随机 nonce
    pub nonce: Vec<u8>,
    /// AES-256-GCM 加密后的密文
    pub ciphertext: Vec<u8>,
    /// 发送方 X25519 公钥（32 字节）
    pub public_key: Vec<u8>,
}

/// 加密会话，存储与远程设备共享的对称密钥
pub struct CryptoSession {
    /// AES-256-GCM 对称密钥（32 字节）
    pub key: [u8; 32],
    /// 远程设备公钥（32 字节）
    pub remote_pubkey: [u8; 32],
}

/// 对 X25519 私钥进行位箝位
fn clamp_scalar(mut scalar: [u8; 32]) -> [u8; 32] {
    scalar[0] &= 248;
    scalar[31] &= 127;
    scalar[31] |= 64;
    scalar
}

/// 根据私钥计算 X25519 公钥
fn compute_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    x25519_dalek::x25519(*private_key, x25519_dalek::X25519_BASEPOINT_BYTES)
}

/// 加密传输层
///
/// 管理 X25519 密钥对，提供加密/解密和会话建立功能。
pub struct CryptoTransport {
    /// 设备身份管理器
    identity: Arc<IdentityManager>,
    /// 本机 X25519 私钥（已箝位）
    local_secret: [u8; 32],
    /// 本机 X25519 公钥
    local_public: [u8; 32],
}

impl CryptoTransport {
    /// 创建加密传输层
    ///
    /// 生成本机 X25519 密钥对。
    pub fn new(identity: Arc<IdentityManager>) -> Self {
        let mut local_secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut local_secret);
        local_secret = clamp_scalar(local_secret);
        let local_public = compute_public_key(&local_secret);
        Self {
            identity,
            local_secret,
            local_public,
        }
    }

    /// 获取本机 X25519 公钥
    pub fn public_key(&self) -> [u8; 32] {
        self.local_public
    }

    /// 获取本机 X25519 私钥字节
    pub fn local_secret_bytes(&self) -> [u8; 32] {
        self.local_secret
    }

    /// 为远程设备建立加密会话
    ///
    /// 使用 ECDH + HKDF 派生 AES-256-GCM 对称密钥。
    pub fn establish_session(&self, remote_public_key: &[u8; 32]) -> CryptoSession {
        let shared = ecdh_shared_secret(&self.local_secret, remote_public_key);
        let key = derive_aes_key(&shared);
        CryptoSession {
            key,
            remote_pubkey: *remote_public_key,
        }
    }

    /// 加密消息 payload
    ///
    /// 使用会话中的对称密钥进行 AES-256-GCM 加密。
    pub fn encrypt(
        &self,
        session: &CryptoSession,
        plaintext: &str,
    ) -> Result<EncryptedPayload, CryptoError> {
        let (nonce, ciphertext) = aes_encrypt(&session.key, plaintext.as_bytes())?;
        let public_key = self.local_public;
        Ok(EncryptedPayload {
            nonce,
            ciphertext,
            public_key: public_key.to_vec(),
        })
    }

    /// 解密消息 payload
    ///
    /// 先尝试用指定 session 解密。
    /// 如果解密失败，用 payload 中的公钥 + local_private_key 重建新 session 再试。
    pub fn decrypt(
        &self,
        session: &mut CryptoSession,
        payload: &EncryptedPayload,
        local_private_key: &[u8; 32],
    ) -> Result<String, CryptoError> {
        // 首先尝试用现有 session 解密
        if payload.nonce.len() == 12 {
            let mut nonce_arr = [0u8; 12];
            nonce_arr.copy_from_slice(&payload.nonce);
            if let Ok(plaintext) = aes_decrypt(&session.key, &nonce_arr, &payload.ciphertext) {
                return String::from_utf8(plaintext)
                    .map_err(|_| CryptoError::DecryptFailed("解密结果不是有效 UTF-8".to_string()));
            }
        }

        // 如果现有 session 无法解密，用 payload 中的公钥重建 session
        if payload.public_key.len() != 32 {
            return Err(CryptoError::InvalidPayload);
        }
        let mut remote_pubkey = [0u8; 32];
        remote_pubkey.copy_from_slice(&payload.public_key);

        let shared = ecdh_shared_secret(local_private_key, &remote_pubkey);
        let key = derive_aes_key(&shared);
        session.key = key;
        session.remote_pubkey = remote_pubkey;

        if payload.nonce.len() != 12 {
            return Err(CryptoError::InvalidPayload);
        }
        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(&payload.nonce);

        let plaintext = aes_decrypt(&session.key, &nonce_arr, &payload.ciphertext)?;
        String::from_utf8(plaintext)
            .map_err(|_| CryptoError::DecryptFailed("解密结果不是有效 UTF-8".to_string()))
    }
}

// ============================================================
// 纯函数（不依赖 CryptoTransport）
// ============================================================

/// 计算 ECDH 共享密钥
///
/// 使用 X25519 曲线进行 Diffie-Hellman 密钥交换。
pub fn ecdh_shared_secret(private_key: &[u8; 32], public_key: &[u8; 32]) -> [u8; 32] {
    x25519_dalek::x25519(*private_key, *public_key)
}

/// HKDF 派生 AES-256-GCM 密钥
///
/// 从 ECDH 共享密钥派生 32 字节对称密钥。
/// - salt: "PaseBoard v0.2"（固定字符串）
/// - info: "transport-key"
pub fn derive_aes_key(shared_secret: &[u8; 32]) -> [u8; 32] {
    let salt = b"PaseBoard v0.2";
    let info = b"transport-key";
    let hk = Hkdf::<Sha256>::new(Some(&salt[..]), &shared_secret[..]);
    let mut okm = [0u8; 32];
    hk.expand(&info[..], &mut okm)
        .expect("HKDF expand 不应失败");
    okm
}

/// 用 AES-256-GCM 加密
///
/// 生成随机 12 字节 nonce，返回 (nonce, ciphertext)。
pub fn aes_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CryptoError::EncryptFailed(format!("密钥初始化失败: {}", e)))?;

    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CryptoError::EncryptFailed(format!("AES-GCM 加密失败: {}", e)))?;

    Ok((nonce_bytes.to_vec(), ciphertext))
}

/// 用 AES-256-GCM 解密
pub fn aes_decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CryptoError::DecryptFailed(format!("密钥初始化失败: {}", e)))?;

    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptFailed("AES-GCM 解密失败".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn generate_test_keypair() -> ([u8; 32], [u8; 32]) {
        let mut sk = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut sk);
        sk = clamp_scalar(sk);
        let pk = compute_public_key(&sk);
        (sk, pk)
    }

    #[test]
    fn test_ecdh_symmetric() {
        let (sk_a, pk_a) = generate_test_keypair();
        let (sk_b, pk_b) = generate_test_keypair();

        let shared_a = ecdh_shared_secret(&sk_a, &pk_b);
        let shared_b = ecdh_shared_secret(&sk_b, &pk_a);

        assert_eq!(shared_a, shared_b, "ECDH 共享密钥应对称");
    }

    #[test]
    fn test_derive_aes_key_deterministic() {
        let shared = [0xABu8; 32];
        let key1 = derive_aes_key(&shared);
        let key2 = derive_aes_key(&shared);
        assert_eq!(key1, key2, "相同共享密钥应派生出相同 AES 密钥");
    }

    #[test]
    fn test_aes_encrypt_decrypt_roundtrip() {
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);

        let plaintext = b"Hello, PaseBoard! AES-256-GCM test";

        let (nonce, ciphertext) = aes_encrypt(&key, plaintext).unwrap();
        assert_eq!(nonce.len(), 12);
        assert_ne!(ciphertext, plaintext, "密文不应与明文相同");

        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(&nonce);
        let decrypted = aes_decrypt(&key, &nonce_arr, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext, "解密结果应与原文一致");
    }

    #[test]
    fn test_aes_decrypt_wrong_key_fails() {
        let mut key1 = [0u8; 32];
        let mut key2 = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key1);
        rand::rngs::OsRng.fill_bytes(&mut key2);

        let plaintext = b"secret data";
        let (nonce, ciphertext) = aes_encrypt(&key1, plaintext).unwrap();

        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(&nonce);
        let result = aes_decrypt(&key2, &nonce_arr, &ciphertext);
        assert!(result.is_err(), "错误密钥解密应失败");
    }

    #[test]
    fn test_crypto_transport_establish_session() {
        let identity_path = std::env::temp_dir().join("paseboard_test_crypto_identity.pem");
        let _ = std::fs::remove_file(&identity_path);

        let identity = Arc::new(IdentityManager::new(identity_path.clone()).unwrap());
        let crypto = CryptoTransport::new(identity);

        let mut remote_pk = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut remote_pk);

        let session = crypto.establish_session(&remote_pk);
        assert_eq!(session.remote_pubkey, remote_pk);
        assert_ne!(session.key, [0u8; 32], "派生的密钥不应全零");

        let _ = std::fs::remove_file(&identity_path);
    }

    #[test]
    fn test_crypto_transport_encrypt_decrypt() {
        let identity_path = std::env::temp_dir().join("paseboard_test_crypto_ed.pem");
        let _ = std::fs::remove_file(&identity_path);

        let identity = Arc::new(IdentityManager::new(identity_path.clone()).unwrap());
        let crypto = CryptoTransport::new(identity);

        let public_key = crypto.public_key();

        // 模拟远程公钥（随机生成）
        let (remote_sk, remote_pk) = generate_test_keypair();
        let session = crypto.establish_session(&remote_pk);

        // 加密消息
        let plaintext = "Hello from device A!";
        let payload = crypto.encrypt(&session, plaintext).unwrap();
        assert_eq!(payload.nonce.len(), 12);
        assert_eq!(payload.public_key.len(), 32);
        assert_eq!(payload.public_key, public_key.to_vec());

        // 解密（重建 session）
        let mut session_b_to_a = CryptoSession {
            key: [0u8; 32],
            remote_pubkey: [0u8; 32],
        };
        let decrypted = crypto
            .decrypt(&mut session_b_to_a, &payload, &remote_sk)
            .unwrap();
        assert_eq!(decrypted, plaintext);
        assert_eq!(session_b_to_a.key, session.key, "session 密钥应一致");

        let _ = std::fs::remove_file(&identity_path);
    }

    #[test]
    fn test_encrypt_decrypt_with_established_session() {
        let identity_path = std::env::temp_dir().join("paseboard_test_crypto_es.pem");
        let _ = std::fs::remove_file(&identity_path);

        let identity = Arc::new(IdentityManager::new(identity_path.clone()).unwrap());
        let crypto = CryptoTransport::new(identity);

        let (_remote_sk, remote_pk) = generate_test_keypair();
        let session = crypto.establish_session(&remote_pk);

        // 使用已有 session 加密和解密
        let plaintext = "Hello, encrypted world!";
        let payload = crypto.encrypt(&session, plaintext).unwrap();

        let local_sk = crypto.local_secret_bytes();
        let mut session_clone = CryptoSession {
            key: session.key,
            remote_pubkey: session.remote_pubkey,
        };
        let decrypted = crypto
            .decrypt(&mut session_clone, &payload, &local_sk)
            .unwrap();
        assert_eq!(decrypted, plaintext);

        let _ = std::fs::remove_file(&identity_path);
    }
}

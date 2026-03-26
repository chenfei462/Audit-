//! 数据加密模块
//!
//! 使用 AES-256-GCM 对财务数据进行加密存储和传输。
//! 支持加密导出和解密导入审计数据。

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// 加密后的数据包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Base64 编码的密文
    pub ciphertext: String,
    /// Base64 编码的 nonce
    pub nonce: String,
    /// 加密时间
    pub encrypted_at: String,
}

/// 从密码派生 256 位密钥（简易版 PBKDF）
fn derive_key(password: &str) -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut key = [0u8; 32];
    // 多轮哈希增强安全性
    let mut data = password.as_bytes().to_vec();
    for i in 0..32 {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        i.hash(&mut hasher);
        let h = hasher.finish().to_le_bytes();
        key[i] = h[i % 8];
        data.extend_from_slice(&h);
    }
    key
}

/// 使用密码加密数据
pub fn encrypt(plaintext: &str, password: &str) -> Result<EncryptedData> {
    let key_bytes = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .context("创建加密器失败")?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("加密失败: {}", e))?;

    Ok(EncryptedData {
        ciphertext: BASE64.encode(&ciphertext),
        nonce: BASE64.encode(&nonce_bytes),
        encrypted_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// 使用密码解密数据
pub fn decrypt(encrypted: &EncryptedData, password: &str) -> Result<String> {
    let key_bytes = derive_key(password);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .context("创建解密器失败")?;

    let ciphertext = BASE64.decode(&encrypted.ciphertext)
        .context("Base64 解码密文失败")?;
    let nonce_bytes = BASE64.decode(&encrypted.nonce)
        .context("Base64 解码 nonce 失败")?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("解密失败：密码错误或数据已损坏"))?;

    String::from_utf8(plaintext).context("UTF-8 解码失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let data = r#"{"test": "财务数据", "amount": 12345.67}"#;
        let password = "MySecretPassword123";

        let encrypted = encrypt(data, password).unwrap();
        assert!(!encrypted.ciphertext.is_empty());

        let decrypted = decrypt(&encrypted, password).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_password() {
        let data = "sensitive data";
        let encrypted = encrypt(data, "correct_password").unwrap();
        let result = decrypt(&encrypted, "wrong_password");
        assert!(result.is_err());
    }
}

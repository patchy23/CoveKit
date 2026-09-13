//! AES-256-GCM 加解密原语（框架唯一实现）
//! - 落盘格式：`nonce(12B) ‖ ciphertext(含 16B 认证标签)`；每次加密使用新 nonce
//! - 解密先校验最小长度（12 + 16），损坏文件报错而不会 panic
//! - Vault 凭证库与 credentials 兼容 KV 命名空间共用本实现，禁止各自再写一份

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

/// 密文最小长度：12 字节 nonce + 16 字节 GCM 认证标签
pub(crate) const MIN_CIPHERTEXT_LEN: usize = 28;

/// 使用主密钥解密 `nonce(12B)‖ciphertext`，先校验最小长度避免损坏文件触发 panic。
pub(crate) fn decrypt_payload(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < MIN_CIPHERTEXT_LEN {
        return Err("凭证文件损坏（密文长度不足）".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let (nonce, ct) = data.split_at(12);
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| "凭证解密失败（主密钥不匹配或数据损坏）".into())
}

/// 使用主密钥加密明文，返回 `nonce(12B)‖ciphertext`（每次新 nonce）。
pub(crate) fn encrypt_payload(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|_| "凭证加密失败")?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 用候选主密钥尝试解密：能通过 GCM 认证即认为这把密钥属于该密文。
/// 主密钥候选选择用它做判据（不依赖任何外部状态）。
pub(crate) fn authenticates(key: &[u8; 32], data: &[u8]) -> bool {
    decrypt_payload(key, data).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 加解密往返 + 错误密钥/损坏密文校验
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [7u8; 32];
        let plain = b"{\"conn-1\":\"s3cret\"}";
        let ct = encrypt_payload(&key, plain).unwrap_or_default();
        assert_eq!(decrypt_payload(&key, &ct).unwrap_or_default(), plain);
        assert!(decrypt_payload(&[8u8; 32], &ct).is_err());
        assert!(decrypt_payload(&key, b"short").is_err());
        assert!(!authenticates(&[8u8; 32], &ct));
        assert!(authenticates(&key, &ct));
    }

    /// nonce 唯一性：同明文同密钥两次加密密文必须不同（否则 AES-GCM nonce 重用是安全事故）
    #[test]
    fn nonce_uniqueness() {
        let key = [7u8; 32];
        let plain = b"same plaintext";
        let a = encrypt_payload(&key, plain).unwrap_or_default();
        let b = encrypt_payload(&key, plain).unwrap_or_default();
        assert_ne!(a, b);
        assert_eq!(decrypt_payload(&key, &a).unwrap_or_default(), plain);
        assert_eq!(decrypt_payload(&key, &b).unwrap_or_default(), plain);
    }

    /// 损坏密文报错不 panic：长度过短与篡改认证标签都必须返回 Err
    #[test]
    fn corrupted_ciphertext_errors_no_panic() {
        let key = [7u8; 32];
        assert!(decrypt_payload(&key, b"short").is_err());
        assert!(decrypt_payload(&key, &[0u8; MIN_CIPHERTEXT_LEN - 1]).is_err());
        let mut ct = encrypt_payload(&key, b"hello vault").unwrap_or_default();
        let last = ct.len() - 1;
        ct[last] ^= 0xFF; // 篡改认证标签
        assert!(decrypt_payload(&key, &ct).is_err());
        // 篡改 nonce 同样必须失败
        let mut ct2 = encrypt_payload(&key, b"hello vault").unwrap_or_default();
        ct2[0] ^= 0xFF;
        assert!(decrypt_payload(&key, &ct2).is_err());
    }
}

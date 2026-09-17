//! AES-256-GCM 加解密原语（框架唯一实现）
//! - 落盘格式：`nonce(12B) ‖ ciphertext(含 16B 认证标签)`；每次加密使用新 nonce
//! - 解密先校验最小长度（12 + 16），损坏文件报错而不会 panic
//! - Vault 凭证库与 credentials 兼容 KV 命名空间共用本实现，禁止各自再写一份
//! - Argon2id 口令派生（`derive_key_argon2id`）同样只此一份：Vault 备份导出与数据包容器共用

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

/// 密文最小长度：12 字节 nonce + 16 字节 GCM 认证标签
pub(crate) const MIN_CIPHERTEXT_LEN: usize = NONCE_LEN + GCM_TAG_LEN;

/// AES-GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;
/// AES-GCM 认证标签长度（字节）
const GCM_TAG_LEN: usize = 16;

/// 使用主密钥解密 `nonce(12B)‖ciphertext`，先校验最小长度避免损坏文件触发 panic。
///
/// 生产路径已全部改用 uid 绑定版（`decrypt_with_aad`）；本函数仅测试构造旧格式夹具用。
#[cfg(test)]
pub(crate) fn decrypt_payload(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < MIN_CIPHERTEXT_LEN {
        return Err("凭证文件损坏（密文长度不足）".into());
    }
    decrypt_with_aad(key, data, &[]).map_err(|_| "凭证解密失败（主密钥不匹配或数据损坏）".into())
}

/// 使用主密钥加密明文，返回 `nonce(12B)‖ciphertext`（每次新 nonce）。
///
/// 生产路径已全部改用 uid 绑定版（`encrypt_with_aad`）；本函数仅测试构造旧格式夹具用。
#[cfg(test)]
pub(crate) fn encrypt_payload(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, String> {
    encrypt_with_aad(key, plain, &[]).map_err(|_| "凭证加密失败".into())
}

/// 带附加认证数据（AAD）的加密：`nonce(12B)‖ciphertext`，AAD 参与认证标签计算。
///
/// 数据包容器用它把明文头（算法参数/salt/nonce/长度）绑定进密文认证，
/// 头被改动一个字节即解密失败，不需要另写一套完整性校验。
pub(crate) fn encrypt_with_aad(
    key: &[u8; 32],
    plain: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = encrypt_with_aad_nonce(key, plain, aad, &nonce)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 带附加认证数据（AAD）的解密（AAD 与加密时不一致即认证失败）。
pub(crate) fn decrypt_with_aad(key: &[u8; 32], data: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < MIN_CIPHERTEXT_LEN {
        return Err("密文长度不足".into());
    }
    let (nonce, ct) = data.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| "密文长度不足".to_string())?;
    decrypt_with_aad_nonce(key, ct, aad, &nonce)
}

/// 指定 nonce 的加密，返回**裸密文**（不含 nonce）。
///
/// 供「nonce 要落进头部、且头部本身是 AAD」的容器使用（数据包 `.pbdata`）：
/// 调用方负责 nonce 的唯一性（每次加密重新随机），本函数不复用也不生成 nonce。
pub(crate) fn encrypt_with_aad_nonce(
    key: &[u8; 32],
    plain: &[u8],
    aad: &[u8],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher
        .encrypt(
            Nonce::from_slice(nonce),
            aes_gcm::aead::Payload { msg: plain, aad },
        )
        .map_err(|_| "加密失败".into())
}

/// 指定 nonce 的解密，入参为**裸密文**（与 `encrypt_with_aad_nonce` 对应）。
pub(crate) fn decrypt_with_aad_nonce(
    key: &[u8; 32],
    data: &[u8],
    aad: &[u8],
    nonce: &[u8; NONCE_LEN],
) -> Result<Vec<u8>, String> {
    if data.len() < GCM_TAG_LEN {
        return Err("密文长度不足".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            aes_gcm::aead::Payload { msg: data, aad },
        )
        .map_err(|_| "解密失败（密钥不匹配或数据损坏）".into())
}

/// 用候选主密钥尝试解密：能通过 GCM 认证即认为这把密钥属于该密文。
/// 主密钥候选选择用它做判据（不依赖任何外部状态）。
/// 生产路径用 `authenticates_with_aad`；本函数仅测试用。
#[cfg(test)]
pub(crate) fn authenticates(key: &[u8; 32], data: &[u8]) -> bool {
    decrypt_payload(key, data).is_ok()
}

/// 带 AAD 的认证校验（空间 uid 绑定版）：密钥与 AAD 同时匹配才为真
pub(crate) fn authenticates_with_aad(key: &[u8; 32], data: &[u8], aad: &[u8]) -> bool {
    decrypt_with_aad(key, data, aad).is_ok()
}

/// Argon2id 口令派生（框架唯一实现）：口令 + 盐 + 成本参数 → 32B 密钥。
///
/// 成本参数的白名单校验属于调用方职责：本函数只负责按传入参数派生，
/// 导入侧必须先验证参数范围再调用（否则等于按攻击者声明的成本分配内存）。
pub(crate) fn derive_key_argon2id(
    password: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; 32], String> {
    let params =
        Params::new(m_cost, t_cost, p_cost, Some(32)).map_err(|e| format!("KDF 参数非法: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("密钥派生失败: {e}"))?;
    Ok(key)
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

    /// AAD 绑定与指定 nonce 的一支：AAD/nonce 任一处不同都必须解密失败（数据包容器靠此把头部纳入认证）
    #[test]
    fn aad_and_explicit_nonce_bind_ciphertext() {
        let key = [9u8; 32];
        let ct = encrypt_with_aad(&key, b"manifest", b"header-a").unwrap_or_default();
        assert_eq!(
            decrypt_with_aad(&key, &ct, b"header-a").unwrap_or_default(),
            b"manifest"
        );
        assert!(decrypt_with_aad(&key, &ct, b"header-b").is_err());

        let nonce = [3u8; 12];
        let raw = encrypt_with_aad_nonce(&key, b"body", b"hdr", &nonce).unwrap_or_default();
        assert_eq!(
            decrypt_with_aad_nonce(&key, &raw, b"hdr", &nonce).unwrap_or_default(),
            b"body"
        );
        assert!(decrypt_with_aad_nonce(&key, &raw, b"other", &nonce).is_err());
        assert!(decrypt_with_aad_nonce(&key, &raw, b"hdr", &[4u8; 12]).is_err());
        assert!(decrypt_with_aad_nonce(&key, b"short", b"hdr", &nonce).is_err());
    }

    /// Argon2id 口令派生可复现：同口令同盐同参数结果一致，换盐即变
    #[test]
    fn argon2id_derivation_is_reproducible() {
        let a =
            derive_key_argon2id("pw-123456", b"0123456789abcdef", 8192, 1, 1).unwrap_or_default();
        let b =
            derive_key_argon2id("pw-123456", b"0123456789abcdef", 8192, 1, 1).unwrap_or_default();
        assert_eq!(a, b);
        let other =
            derive_key_argon2id("pw-123456", b"fedcba9876543210", 8192, 1, 1).unwrap_or_default();
        assert_ne!(a, other);
    }
}

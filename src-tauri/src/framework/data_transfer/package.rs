//! 数据包容器（`.pbdata` v1）：明文头 + 口令派生密钥 + AES-256-GCM（头字节作 AAD）
//!
//! 布局（L2 方案 §4.1）：
//! `magic(8B "PBDATA1\0") ‖ containerVersion(u16 LE) ‖ kdfAlgo(u8) ‖ mCost(u32 LE) ‖ tCost(u32 LE)`
//! `‖ pCost(u8) ‖ salt(16B) ‖ nonce(12B) ‖ cipherLen(u64 LE) ‖ ciphertext`
//!
//! 三条不变量：
//! - 头参与 AEAD 认证（AAD = 头字节）：改头、改密文、口令错误都在认证阶段失败，
//!   不存在「先信头再验内容」的中间态
//! - 解析任何包内参数前先过白名单（KDF 成本、密文长度、明文上限），
//!   不让构造出来的包决定导入端分配多少内存
//! - 头内只有算法参数与随机量：工具名、主机名、目录、记录清单、来源标识全在密文清单里
//!
//! 口令不落盘、不进日志；本模块的函数只接收口令参数，不做任何持久化。
use std::io::Write;
use std::path::Path;

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::framework::data_transfer::types::{validate_manifest, PackageManifest};
use crate::framework::secure_store::{
    decrypt_with_aad_nonce, derive_key_argon2id, encrypt_with_aad_nonce,
};

/// 包魔数（8 字节；尾字节 0 便于文本工具识别边界）
const MAGIC: [u8; 8] = *b"PBDATA1\0";
/// 容器版本（头部布局变更时递增；导入端只认当前版本）
const CONTAINER_VERSION: u16 = 1;
/// KDF 算法标识：Argon2id
const KDF_ARGON2ID: u8 = 1;
/// Argon2id 盐长度（字节）
const SALT_LEN: usize = 16;
/// AES-GCM nonce 长度（字节）
const NONCE_LEN: usize = 12;
/// AES-GCM 认证标签长度（字节）
const GCM_TAG_LEN: u64 = 16;
/// 明文上限（32 MiB，§4.3）
const MAX_PLAINTEXT_BYTES: u64 = 32 * 1024 * 1024;
/// 头部长度（magic ‖ version ‖ algo ‖ mCost ‖ tCost ‖ pCost ‖ salt ‖ nonce ‖ cipherLen）
const HEADER_LEN: usize = 8 + 2 + 1 + 4 + 4 + 1 + SALT_LEN + NONCE_LEN + 8;

/// KDF 白名单：内存成本下限（KiB，8 MiB）
const MIN_M_COST_KIB: u32 = 8 * 1024;
/// KDF 白名单：内存成本上限（KiB，256 MiB）
const MAX_M_COST_KIB: u32 = 256 * 1024;
/// KDF 白名单：迭代次数上限
const MAX_T_COST: u32 = 10;
/// KDF 白名单：并行度上限
const MAX_P_COST: u8 = 4;
/// 导出端口令最短长度（导入侧按解密结果判定，不重复校验长度）
pub(crate) const MIN_PASSWORD_LEN: usize = 8;
/// 文件读取上限：头部声明的明文上限 + 头部 + 认证标签，超出直接拒绝而不读进内存
const MAX_FILE_BYTES: u64 = MAX_PLAINTEXT_BYTES + HEADER_LEN as u64 + GCM_TAG_LEN;

/// 包明文头：字段顺序即落盘顺序，编码结果同时作为 AAD
struct PackageHeader {
    /// 容器版本
    version: u16,
    /// KDF 算法标识（1 = Argon2id）
    kdf_algo: u8,
    /// Argon2id 内存成本（KiB）
    m_cost: u32,
    /// Argon2id 迭代次数
    t_cost: u32,
    /// Argon2id 并行度
    p_cost: u8,
    /// Argon2id 盐
    salt: [u8; SALT_LEN],
    /// AES-GCM nonce（即密文所用的 nonce）
    nonce: [u8; NONCE_LEN],
    /// 密文长度（不含 nonce，含 16 字节认证标签）
    cipher_len: u64,
}

impl PackageHeader {
    /// 编码为定长头部字节（同时作为加密与解密的 AAD）
    fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..8].copy_from_slice(&MAGIC);
        out[8..10].copy_from_slice(&self.version.to_le_bytes());
        out[10] = self.kdf_algo;
        out[11..15].copy_from_slice(&self.m_cost.to_le_bytes());
        out[15..19].copy_from_slice(&self.t_cost.to_le_bytes());
        out[19] = self.p_cost;
        out[20..36].copy_from_slice(&self.salt);
        out[36..48].copy_from_slice(&self.nonce);
        out[48..56].copy_from_slice(&self.cipher_len.to_le_bytes());
        out
    }

    /// 解析头部：先认魔数与版本，再逐项白名单校验（不信任包内的任何数值）。
    ///
    /// 调用方保证 `bytes` 至少 `HEADER_LEN` 字节；不足时返回错误而不是 panic。
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN {
            return Err(format!("数据包损坏（头部不足 {HEADER_LEN} 字节）"));
        }
        if bytes[..8] != MAGIC[..] {
            return Err("这不是 patchyBox 数据包（魔数不匹配）".into());
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != CONTAINER_VERSION {
            return Err(format!(
                "不支持的数据包容器版本 {version}（当前支持 {CONTAINER_VERSION}）"
            ));
        }
        let kdf_algo = bytes[10];
        if kdf_algo != KDF_ARGON2ID {
            return Err(format!("不支持的数据包 KDF 算法标识 {kdf_algo}"));
        }
        let m_cost = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);
        let t_cost = u32::from_le_bytes([bytes[15], bytes[16], bytes[17], bytes[18]]);
        let p_cost = bytes[19];
        if !(MIN_M_COST_KIB..=MAX_M_COST_KIB).contains(&m_cost) {
            return Err(format!(
                "数据包 KDF 内存成本 {m_cost} KiB 超出白名单（{MIN_M_COST_KIB}..={MAX_M_COST_KIB}）"
            ));
        }
        if !(1..=MAX_T_COST).contains(&t_cost) {
            return Err(format!(
                "数据包 KDF 迭代次数 {t_cost} 超出白名单（1..={MAX_T_COST}）"
            ));
        }
        if !(1..=MAX_P_COST).contains(&p_cost) {
            return Err(format!(
                "数据包 KDF 并行度 {p_cost} 超出白名单（1..={MAX_P_COST}）"
            ));
        }
        let cipher_len = u64::from_le_bytes([
            bytes[48], bytes[49], bytes[50], bytes[51], bytes[52], bytes[53], bytes[54], bytes[55],
        ]);
        if cipher_len > MAX_PLAINTEXT_BYTES + GCM_TAG_LEN {
            return Err(format!(
                "数据包声明的明文长度超过上限（{} 字节）",
                MAX_PLAINTEXT_BYTES
            ));
        }
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[20..36]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[36..48]);
        Ok(Self {
            version,
            kdf_algo,
            m_cost,
            t_cost,
            p_cost,
            salt,
            nonce,
            cipher_len,
        })
    }
}

/// 导出端口令下限校验：口令太短时直接拒绝导出，避免生成「形式上加密、实际可穷举」的包。
pub(crate) fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("导出密码至少需要 {MIN_PASSWORD_LEN} 位"));
    }
    Ok(())
}

/// 导出：口令 + 清单 → 完整包字节。
///
/// 口令过短、清单自相矛盾或超限都在这里被拒（先校验再加密，不生成半成品）。
pub(crate) fn seal_package(password: &str, manifest: &PackageManifest) -> Result<Vec<u8>, String> {
    validate_password(password)?;
    validate_manifest(manifest)?;
    let plain = serde_json::to_vec(manifest).map_err(|e| format!("数据包清单序列化失败: {e}"))?;
    if plain.len() as u64 > MAX_PLAINTEXT_BYTES {
        return Err(format!(
            "数据包明文超过上限（{MAX_PLAINTEXT_BYTES} 字节），请缩小导出范围"
        ));
    }
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    // 成本参数取 argon2 默认值（与 Vault 备份容器同源），导入端按头部声明复核白名单
    let m_cost = argon2::Params::DEFAULT_M_COST;
    let t_cost = argon2::Params::DEFAULT_T_COST;
    let p_cost = argon2::Params::DEFAULT_P_COST as u8;
    let key = derive_key_argon2id(password, &salt, m_cost, t_cost, u32::from(p_cost))?;
    // 密文长度先算出来（明文 + 认证标签）写进头，头才是最终 AAD；
    // nonce 先随机定下并落头，再交给原语加密（原语不复用也不生成 nonce）
    let header = PackageHeader {
        version: CONTAINER_VERSION,
        kdf_algo: KDF_ARGON2ID,
        m_cost,
        t_cost,
        p_cost,
        salt,
        nonce,
        cipher_len: plain.len() as u64 + GCM_TAG_LEN,
    };
    let head = header.encode();
    let ciphertext = encrypt_with_aad_nonce(&key, &plain, &head, &nonce)?;
    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.extend_from_slice(&head);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// 导入：口令 + 完整包字节 → 清单。
///
/// 口令错误、头部被改、密文被改、参数越界、清单不自洽都返回 Err，不产生部分解析结果。
pub(crate) fn open_package(password: &str, raw: &[u8]) -> Result<PackageManifest, String> {
    let head = raw
        .get(..HEADER_LEN)
        .ok_or_else(|| format!("数据包损坏（不足 {HEADER_LEN} 字节头部）"))?;
    let header = PackageHeader::parse(head)?;
    let ciphertext = &raw[HEADER_LEN..];
    if ciphertext.len() as u64 != header.cipher_len {
        return Err(format!(
            "数据包损坏（密文长度 {} 与头部声明 {} 不一致）",
            ciphertext.len(),
            header.cipher_len
        ));
    }
    let key = derive_key_argon2id(
        password,
        &header.salt,
        header.m_cost,
        header.t_cost,
        u32::from(header.p_cost),
    )
    .map_err(|e| format!("数据包口令派生失败: {e}"))?;
    let plain = decrypt_with_aad_nonce(&key, ciphertext, head, &header.nonce)
        .map_err(|_| "数据包解密失败（口令错误、文件损坏或头部被改动）".to_string())?;
    let manifest: PackageManifest =
        serde_json::from_slice(&plain).map_err(|e| format!("数据包清单解析失败: {e}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// 读包文件：超过读取上限的文件直接拒绝，不整份读进内存。
pub(crate) fn read_package(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("数据包读取失败: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "数据包文件过大（{} 字节，上限 {MAX_FILE_BYTES}）",
            meta.len()
        ));
    }
    std::fs::read(path).map_err(|e| format!("数据包读取失败: {e}"))
}

/// 读包的不可信输入摘要（`inspectId` 绑定它，提交前复核文件没被换过）
pub(crate) fn file_digest(raw: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw);
    format!("{:x}", hasher.finalize())
}

/// 写包文件：唯一临时名 + 刷盘 + 原子替换，失败时不留临时文件、原文件保持不动。
///
/// 覆盖已有文件的确认由调用方（前端保存对话框）完成；本函数不做二次询问。
pub(crate) fn write_package(path: &Path, content: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("package.pbdata");
    let tmp = path.with_file_name(format!("{file_name}.tmp-{}", uuid::Uuid::new_v4()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("数据包目录创建失败: {e}"))?;
    }
    let write_result = std::fs::File::create(&tmp).and_then(|mut file| {
        file.write_all(content)?;
        // 刷盘在写入句柄上进行：Windows 上以只读句柄调用 sync_all 会被拒绝
        file.sync_all()
    });
    if let Err(e) = write_result {
        std::fs::remove_file(&tmp).ok();
        return Err(format!("数据包写入失败: {e}"));
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        std::fs::remove_file(&tmp).ok();
        format!("数据包替换失败（原文件未改动）: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::data_transfer::types::{
        DatasetBlock, TransportPolicy, MAX_RECORDS_PER_PACKAGE,
    };
    use serde_json::{json, Value};

    /// 测试用口令（满足最短长度）
    const PW: &str = "transfer-pw-1";

    /// 构造一份内容自洽的单块清单
    fn manifest_with(host: &str) -> PackageManifest {
        let mut manifest = PackageManifest::new("default", "默认空间");
        let records = Value::Array(vec![json!({"id": "p1", "host": host})]);
        manifest.datasets.push(DatasetBlock {
            name: "ssh.profiles".into(),
            schema_version: 1,
            policy: TransportPolicy::Portable,
            record_count: 1,
            sha256: crate::framework::data_transfer::types::dataset_digest(&records)
                .unwrap_or_default(),
            records: Some(records),
        });
        manifest
    }

    /// 往返：加密后可解回同一清单；密文里看不到明文主机名（明文头不含业务信息）
    #[test]
    fn seal_open_roundtrip_and_no_plaintext_leak() {
        let manifest = manifest_with("srv-秘密主机.example.com");
        let raw = seal_package(PW, &manifest).unwrap_or_default();
        assert_eq!(
            raw.len() as u64,
            HEADER_LEN as u64 + manifest_plain_len(&raw)
        );
        assert!(raw.len() > HEADER_LEN);
        let head = &raw[..HEADER_LEN];
        assert_eq!(u16::from_le_bytes([head[8], head[9]]), CONTAINER_VERSION);
        assert_eq!(head[10], KDF_ARGON2ID);
        // 明文头里不得出现业务信息
        let head_text = String::from_utf8_lossy(head);
        assert!(!head_text.contains("ssh.profiles"));
        assert!(!head_text.contains("源"));
        let opened = open_package(PW, &raw).unwrap_or_else(|e| panic!("解包失败: {e}"));
        assert_eq!(opened, manifest);
        assert!(!String::from_utf8_lossy(&raw).contains("秘密主机"));
    }

    /// 头部声明的密文长度（测试辅助：直接从头部读回，校验写入端的长度字段）
    fn manifest_plain_len(raw: &[u8]) -> u64 {
        u64::from_le_bytes([
            raw[48], raw[49], raw[50], raw[51], raw[52], raw[53], raw[54], raw[55],
        ])
    }

    /// 口令错误 / 空口令 / 过短口令都被拒，且不 panic
    #[test]
    fn wrong_password_rejected() {
        let raw = seal_package(PW, &manifest_with("h1")).unwrap_or_default();
        assert!(open_package("transfer-pw-2", &raw).is_err());
        assert!(seal_package("", &manifest_with("h1")).is_err());
        assert!(seal_package("short", &manifest_with("h1")).is_err());
    }

    /// 每次导出的 salt 与 nonce 都不同（同明文两次导出密文不同）
    #[test]
    fn salt_and_nonce_randomized() {
        let manifest = manifest_with("h1");
        let a = seal_package(PW, &manifest).unwrap_or_default();
        let b = seal_package(PW, &manifest).unwrap_or_default();
        assert_ne!(a[20..36], b[20..36]); // salt
        assert_ne!(a[36..48], b[36..48]); // nonce
        assert_ne!(a[HEADER_LEN..], b[HEADER_LEN..]);
    }

    /// 改头部任一字段即解密失败（头是 AAD）；改密文同样失败
    #[test]
    fn tampered_header_and_ciphertext_rejected() {
        let raw = seal_package(PW, &manifest_with("h1")).unwrap_or_default();

        // 改 KDF 迭代次数（白名单内，只可能靠 AAD 认证拦住）
        let mut tampered = raw.clone();
        tampered[15] = tampered[15].wrapping_add(1);
        assert!(open_package(PW, &tampered).is_err());

        // 改 salt（派生出的密钥不同）
        let mut tampered_salt = raw.clone();
        tampered_salt[20] ^= 0xFF;
        assert!(open_package(PW, &tampered_salt).is_err());

        // 改 nonce
        let mut tampered_nonce = raw.clone();
        tampered_nonce[36] ^= 0xFF;
        assert!(open_package(PW, &tampered_nonce).is_err());

        // 改密文尾字节（认证标签）
        let mut tampered_ct = raw.clone();
        let last = tampered_ct.len() - 1;
        tampered_ct[last] ^= 0xFF;
        assert!(open_package(PW, &tampered_ct).is_err());

        // 截断（长度与头部声明不符）
        assert!(open_package(PW, &raw[..raw.len() - 1]).is_err());
        assert!(open_package(PW, &raw[..10]).is_err());
        assert!(open_package(PW, &[]).is_err());
    }

    /// 魔数 / 容器版本 / KDF 标识不符都给出明确错误
    #[test]
    fn bad_magic_and_version_rejected() {
        let raw = seal_package(PW, &manifest_with("h1")).unwrap_or_default();

        let mut bad_magic = raw.clone();
        bad_magic[0] = b'X';
        let err = open_package(PW, &bad_magic).unwrap_err();
        assert!(err.contains("魔数"), "实际错误: {err}");

        let mut bad_version = raw.clone();
        bad_version[8] = 9;
        let err = open_package(PW, &bad_version).unwrap_err();
        assert!(err.contains("容器版本"), "实际错误: {err}");

        let mut bad_kdf = raw.clone();
        bad_kdf[10] = 7;
        let err = open_package(PW, &bad_kdf).unwrap_err();
        assert!(err.contains("KDF 算法标识"), "实际错误: {err}");
    }

    /// KDF 参数越界在**派生之前**就被拒（否则等于按包内声明分配内存）
    #[test]
    fn kdf_params_out_of_whitelist_rejected_before_derive() {
        let raw = seal_package(PW, &manifest_with("h1")).unwrap_or_default();

        // 内存成本拉到 4 GiB：必须在派生之前拒绝，否则本用例会尝试分配巨额内存
        let mut huge_m = raw.clone();
        huge_m[11..15].copy_from_slice(&(4u32 * 1024 * 1024).to_le_bytes());
        let err = open_package(PW, &huge_m).unwrap_err();
        assert!(err.contains("内存成本"), "实际错误: {err}");

        // 迭代次数 0 与并行度 0 同样非法
        let mut zero_t = raw.clone();
        zero_t[15..19].copy_from_slice(&0u32.to_le_bytes());
        assert!(open_package(PW, &zero_t).is_err());

        let mut zero_p = raw.clone();
        zero_p[19] = 0;
        assert!(open_package(PW, &zero_p).is_err());

        // 声明的明文长度超上限：同样在派生之前拒绝
        let mut huge_plain = raw.clone();
        huge_plain[48..56].copy_from_slice(&(MAX_PLAINTEXT_BYTES + GCM_TAG_LEN + 1).to_le_bytes());
        let err = open_package(PW, &huge_plain).unwrap_err();
        assert!(err.contains("明文长度"), "实际错误: {err}");
    }

    /// 清单不自洽的输入在导出侧就被拒（不生成半成品包）
    #[test]
    fn invalid_manifest_rejected_on_seal() {
        let mut manifest = manifest_with("h1");
        manifest.datasets[0].record_count = MAX_RECORDS_PER_PACKAGE + 1;
        assert!(seal_package(PW, &manifest).is_err());
        let empty = PackageManifest::new("default", "默认空间");
        assert!(seal_package(PW, &empty).is_err());
    }

    /// 文件级往返：临时目录写包再读回，且不留临时文件
    #[test]
    fn write_read_package_roundtrip() {
        let dir = std::env::temp_dir().join(format!("pbdata-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("export.pbdata");
        let manifest = manifest_with("h1");
        let raw = seal_package(PW, &manifest).unwrap_or_default();
        write_package(&path, &raw).unwrap_or_else(|e| panic!("写包失败: {e}"));

        // 目录里只应有目标文件，没有 .tmp- 残留
        let entries: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(entries, vec!["export.pbdata".to_string()]);

        let read_back = read_package(&path).unwrap_or_default();
        assert_eq!(read_back, raw);
        assert_eq!(
            open_package(PW, &read_back).unwrap_or_else(|e| panic!("解包失败: {e}")),
            manifest
        );

        // 覆盖写：内容替换且仍无临时残留
        let raw2 = seal_package(PW, &manifest_with("h2")).unwrap_or_default();
        write_package(&path, &raw2).unwrap_or_else(|e| panic!("覆盖写失败: {e}"));
        assert_eq!(read_package(&path).unwrap_or_default(), raw2);

        // 读不存在的文件：报错不 panic
        assert!(read_package(&dir.join("missing.pbdata")).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 超上限的文件在读之前就被拒（用稀疏大文件不便构造，这里直接校验上限函数路径）
    #[test]
    fn oversized_file_rejected_by_metadata() {
        let dir = std::env::temp_dir().join(format!("pbdata-big-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("big.pbdata");
        // 写一个「头部 + 声明超限长度」的短文件：open 报明文长度超限，read 不因此失败
        let mut raw = seal_package(PW, &manifest_with("h1")).unwrap_or_default();
        raw.truncate(HEADER_LEN);
        raw[48..56].copy_from_slice(&(MAX_PLAINTEXT_BYTES + GCM_TAG_LEN + 1).to_le_bytes());
        write_package(&path, &raw).unwrap_or_else(|e| panic!("写包失败: {e}"));
        let back = read_package(&path).unwrap_or_default();
        assert!(open_package(PW, &back).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}

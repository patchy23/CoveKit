//! 连接凭据加密存储（iota_stronghold 快照引擎）
//! 说明：tauri-plugin-stronghold 的 Rust 侧 API 是前端导向的（命令层在 JS），
//! 插件内部即使用本引擎（iota_stronghold）；数据库插件在 Rust 侧直连引擎，
//! 密钥与解密流程完全不经过前端。主密钥为随机 32 字节（存 db-master.key，
//! 与 ssh 插件 master key 同模式）；凭据按连接 id 存 vault，写后立即提交快照。

use std::path::PathBuf;
use std::sync::Mutex;

use iota_stronghold::{Client, KeyProvider, SnapshotPath};
use rand::RngCore;
use tauri::Manager;
use zeroize::Zeroizing;

/// 快照文件名（app_data_dir 下）
const SNAPSHOT_FILE: &str = "db.stronghold";
/// 主密钥文件名（随机 32 字节）
const MASTER_KEY_FILE: &str = "db-master.key";
/// 客户端路径（进程内唯一）
const CLIENT_PATH: &str = "db-client";

/// 凭据库 State（惰性打开）
pub struct SecretsState(pub Mutex<Option<std::sync::Arc<DbSecrets>>>);

/// stronghold 凭据库（快照 + 密钥提供者）
pub struct DbSecrets {
    /// iota_stronghold 实例（快照在内存中，commit 时落盘）
    stronghold: iota_stronghold::Stronghold,
    /// 快照路径
    snapshot: SnapshotPath,
    /// 密钥提供者（主密钥派生）
    key_provider: KeyProvider,
    /// 客户端（vault/store 操作入口）
    client: Client,
}

impl DbSecrets {
    /// 打开（或首次创建）凭据库
    fn open(app: &tauri::AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("数据目录获取失败: {e}"))?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;

        let key = master_key(&dir)?;
        let snapshot = SnapshotPath::from_path(secrets_path(app)?);
        let stronghold = iota_stronghold::Stronghold::default();
        let key_provider = KeyProvider::try_from(Zeroizing::new(key.to_vec()))
            .map_err(|e| format!("密钥提供者创建失败: {e}"))?;
        if snapshot.exists() {
            stronghold
                .load_snapshot(&key_provider, &snapshot)
                .map_err(|e| format!("凭据快照加载失败: {e}"))?;
        }
        let client = stronghold
            .create_client(CLIENT_PATH)
            .map_err(|e| format!("凭据客户端创建失败: {e}"))?;
        Ok(Self {
            stronghold,
            snapshot,
            key_provider,
            client,
        })
    }

    /// 提交快照（写入/删除凭据后调用）
    fn commit(&self) -> Result<(), String> {
        self.stronghold
            .commit_with_keyprovider(&self.snapshot, &self.key_provider)
            .map_err(|e| format!("凭据快照提交失败: {e}"))
    }

    /// 保存连接密码（store 键值存储；vault 的 read_secret 仅测试编译，生产不可用）
    fn save(&self, conn_id: &str, password: &str) -> Result<(), String> {
        self.client
            .store()
            .insert(
                conn_id.as_bytes().to_vec(),
                password.as_bytes().to_vec(),
                None,
            )
            .map_err(|e| format!("凭据写入失败: {e}"))?;
        self.commit()
    }

    /// 读取连接密码（无记录返回 None）
    fn get(&self, conn_id: &str) -> Result<Option<String>, String> {
        let secret = self
            .client
            .store()
            .get(conn_id.as_bytes())
            .map_err(|e| format!("凭据读取失败: {e}"))?;
        Ok(secret.map(|s| String::from_utf8_lossy(&s).to_string()))
    }

    /// 删除连接密码
    fn delete(&self, conn_id: &str) -> Result<(), String> {
        self.client
            .store()
            .delete(conn_id.as_bytes())
            .map_err(|e| format!("凭据删除失败: {e}"))?;
        self.commit()
    }
}

/// 取（或首次打开）凭据库
fn secrets(
    app: &tauri::AppHandle,
    state: &SecretsState,
) -> Result<std::sync::Arc<DbSecrets>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(std::sync::Arc::new(DbSecrets::open(app)?));
    }
    Ok(guard.clone().expect("已初始化"))
}

/// 主密钥：已存在则读取，否则生成 32 随机字节落盘（一次性）
fn master_key(dir: &std::path::Path) -> Result<[u8; 32], String> {
    let path = dir.join(MASTER_KEY_FILE);
    if path.exists() {
        let bytes = std::fs::read(&path).map_err(|e| format!("主密钥读取失败: {e}"))?;
        if bytes.len() != 32 {
            return Err("主密钥文件损坏（长度不是 32 字节），为避免凭据丢失已停止操作".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, key).map_err(|e| format!("主密钥写入失败: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("主密钥落位失败: {e}"))?;
    Ok(key)
}

/// 凭据库文件路径（供前端展示/诊断）
pub fn secrets_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))?;
    Ok(dir.join(SNAPSHOT_FILE))
}

/// 保存连接密码（连接保存/更新时调用）
pub fn secret_save(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
    password: &str,
) -> Result<(), String> {
    secrets(app, state)?.save(conn_id, password)
}

/// 读取连接密码（连接/测试连接时调用；无记录返回空串）
pub fn secret_get(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
) -> Result<String, String> {
    Ok(secrets(app, state)?.get(conn_id)?.unwrap_or_default())
}

/// 删除连接密码（删除连接时调用）
pub fn secret_delete(
    app: &tauri::AppHandle,
    state: &SecretsState,
    conn_id: &str,
) -> Result<(), String> {
    secrets(app, state)?.delete(conn_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 主密钥文件生成与复用（用临时目录，不触碰真实数据目录）
    #[test]
    fn 主密钥生成后复用且校验长度() {
        let dir = std::env::temp_dir().join(format!("dbx-key-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key1 = master_key(&dir).unwrap();
        let key2 = master_key(&dir).unwrap();
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
        // 损坏的主密钥应报错
        std::fs::write(dir.join(MASTER_KEY_FILE), b"short").unwrap();
        assert!(master_key(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 凭据加解密往返（内存快照：临时文件路径）
    #[test]
    fn 凭据保存与读取往返() {
        let dir = std::env::temp_dir().join(format!("dbx-secrets-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = [7u8; 32];
        std::fs::write(dir.join(MASTER_KEY_FILE), key).unwrap();
        let snapshot = SnapshotPath::from_path(dir.join(SNAPSHOT_FILE));
        let stronghold = iota_stronghold::Stronghold::default();
        let key_provider = KeyProvider::try_from(Zeroizing::new(key.to_vec())).unwrap();
        let client = stronghold.create_client(CLIENT_PATH).unwrap();

        let secrets = DbSecrets {
            stronghold,
            snapshot,
            key_provider,
            client,
        };
        secrets.save("conn-1", "s3cret").unwrap();
        assert_eq!(secrets.get("conn-1").unwrap().as_deref(), Some("s3cret"));
        assert_eq!(secrets.get("conn-missing").unwrap(), None);
        secrets.delete("conn-1").unwrap();
        assert_eq!(secrets.get("conn-1").unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}

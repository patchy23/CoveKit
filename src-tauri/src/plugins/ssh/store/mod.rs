//! SSH 插件 · 服务器配置与分组持久化（插件私有 ssh.db，PluginDb 骨架）
//! 档案保存 Vault 引用或使用本库独立的本地认证表；本地明文保存为用户显式选择。
//! SQL 与行映射写在 &Connection 层（便于内存库单测），PluginDb 仅承担打开/迁移/锁。

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::framework::store::PluginDb;

/// 顺序迁移（只追加）：v1 建分组与服务器配置表
pub(crate) const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS ssh_groups (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       sort_order INTEGER NOT NULL DEFAULT 0
     );
     CREATE TABLE IF NOT EXISTS ssh_profiles (
       id TEXT PRIMARY KEY,
       name TEXT NOT NULL,
       host TEXT NOT NULL,
       port INTEGER NOT NULL,
       username TEXT NOT NULL,
       auth_method TEXT NOT NULL,
       credential_ref TEXT,
       group_id TEXT,
       remark TEXT,
       last_connected_at INTEGER,
       created_at INTEGER NOT NULL,
       updated_at INTEGER NOT NULL
     );",
    // v2：隧道配置表
    "CREATE TABLE IF NOT EXISTS ssh_tunnels (
       id TEXT PRIMARY KEY,
       profile_id TEXT NOT NULL,
       name TEXT NOT NULL,
       tunnel_type TEXT NOT NULL,
       listen_host TEXT NOT NULL,
       listen_port INTEGER NOT NULL,
       target_host TEXT,
       target_port INTEGER,
       auto_start INTEGER NOT NULL DEFAULT 0,
       created_at INTEGER NOT NULL
     );",
    // v3：目录书签表（按服务器隔离）
    "CREATE TABLE IF NOT EXISTS profile_bookmarks (
       id TEXT PRIMARY KEY,
       profile_id TEXT NOT NULL,
       name TEXT NOT NULL,
       path TEXT NOT NULL,
       sort INTEGER NOT NULL DEFAULT 0
     );",
    // v4：用户选择的本地明文认证，独立于 Vault，不进入逻辑配置导出。
    "CREATE TABLE ssh_local_auth (
       profile_id TEXT PRIMARY KEY,
       host TEXT NOT NULL, port INTEGER NOT NULL, username TEXT NOT NULL,
       auth_method TEXT NOT NULL,
       password TEXT, private_key TEXT, passphrase TEXT
     );",
];

/// profile/分组库的惰性句柄（首次访问时打开并迁移）
pub(crate) struct ProfileState(pub Mutex<Option<Arc<PluginDb>>>);

/// 惰性打开插件库（ssh.db；首次调用时执行迁移；打开失败不污染槽位，下次调用重试）
pub(crate) fn with_db<T>(
    app: &AppHandle,
    state: &State<'_, ProfileState>,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let arc = {
        let mut slot = state.0.lock().map_err(|e| e.to_string())?;
        if let Some(db) = slot.as_ref() {
            db.clone()
        } else {
            let db = Arc::new(PluginDb::open(app, "ssh", MIGRATIONS)?);
            *slot = Some(db.clone());
            db
        }
    };
    arc.with_conn(f)
}

/// 打开内存库并跑同一份迁移（单测用）
#[cfg(test)]
pub(crate) fn open_memory() -> Connection {
    let mut conn = Connection::open_in_memory().expect("内存库打开失败");
    crate::framework::store::migrate(&mut conn, MIGRATIONS).expect("内存库迁移失败");
    conn
}

/* ── 分组 ── */

pub(crate) mod bookmarks;
pub(crate) mod local_auth;
pub(crate) mod profiles;
pub(crate) mod tunnels;

pub(crate) use profiles::{get_profile, touch_last_connected};
pub(crate) use tunnels::{delete_tunnel, get_tunnel, list_tunnels};

#[cfg(test)]
mod tests {
    use super::open_memory;
    use super::profiles::*;
    use crate::plugins::ssh::models::{AuthMethod, ServerProfile, SshGroup};

    fn profile(id: &str, group_id: Option<&str>) -> ServerProfile {
        ServerProfile {
            id: id.into(),
            name: format!("服务器 {id}"),
            host: "10.0.0.5".into(),
            port: 22,
            username: "root".into(),
            auth_method: AuthMethod::Password,
            credential_ref: Some("cred-1".into()),
            has_local_auth: false,
            group_id: group_id.map(Into::into),
            remark: None,
            last_connected_at: None,
        }
    }

    #[test]
    fn profile_upsert_get_roundtrip() {
        let conn = open_memory();
        upsert_profile(&conn, &profile("p1", None), 100).unwrap();
        let loaded = get_profile(&conn, "p1").unwrap();
        assert_eq!(loaded.name, "服务器 p1");
        assert_eq!(loaded.credential_ref.as_deref(), Some("cred-1"));
        // 更新：同 id 覆盖 credentialRef 与字段
        let mut updated = profile("p1", None);
        updated.credential_ref = Some("cred-2".into());
        updated.port = 2222;
        upsert_profile(&conn, &updated, 200).unwrap();
        let reloaded = list_profiles(&conn).unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].port, 2222);
        assert_eq!(reloaded[0].credential_ref.as_deref(), Some("cred-2"));
    }

    #[test]
    fn group_delete_moves_profiles_to_ungrouped() {
        let conn = open_memory();
        upsert_group(
            &conn,
            &SshGroup {
                id: "g1".into(),
                name: "生产".into(),
                sort_order: 1,
            },
        )
        .unwrap();
        upsert_profile(&conn, &profile("p1", Some("g1")), 100).unwrap();
        delete_group(&conn, "g1").unwrap();
        assert!(list_groups(&conn).unwrap().is_empty());
        assert_eq!(get_profile(&conn, "p1").unwrap().group_id, None);
    }

    #[test]
    fn auth_method_survives_roundtrip() {
        let conn = open_memory();
        let mut p = profile("p2", None);
        p.auth_method = AuthMethod::PrivateKeyWithPassphrase;
        upsert_profile(&conn, &p, 1).unwrap();
        assert_eq!(
            get_profile(&conn, "p2").unwrap().auth_method,
            AuthMethod::PrivateKeyWithPassphrase
        );
    }

    /// 删除配置必须级联清掉书签与隧道配置行，否则产生孤儿行
    #[test]
    fn delete_profile_cascades_bookmarks_and_tunnels() {
        let conn = open_memory();
        upsert_profile(&conn, &profile("p1", None), 100).unwrap();
        conn.execute(
            "INSERT INTO profile_bookmarks (profile_id, path, name) VALUES ('p1', '/var/log', '日志')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ssh_tunnels (id, profile_id, name, tunnel_type, listen_host, listen_port, target_host, target_port, auto_start, created_at)
             VALUES ('t1', 'p1', '隧道1', 'local', '127.0.0.1', 8080, '127.0.0.1', 80, 0, 100)",
            [],
        )
        .unwrap();
        delete_profile(&conn, "p1").unwrap();
        let orphans: i64 = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM profile_bookmarks WHERE profile_id = 'p1')
                        + (SELECT COUNT(*) FROM ssh_tunnels WHERE profile_id = 'p1')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
        assert!(get_profile(&conn, "p1").is_err());
    }

    /// 编辑保存不带回 last_connected_at（None）时必须保留库里的原值
    #[test]
    fn upsert_profile_preserves_last_connected_at() {
        let conn = open_memory();
        let mut p = profile("p1", None);
        p.last_connected_at = Some(1_758_000_000_000);
        upsert_profile(&conn, &p, 100).unwrap();
        // 模拟编辑保存：字段变更，但没带回 last_connected_at
        p.last_connected_at = None;
        p.remark = Some("改过备注".into());
        upsert_profile(&conn, &p, 200).unwrap();
        let saved = get_profile(&conn, "p1").unwrap();
        assert_eq!(saved.last_connected_at, Some(1_758_000_000_000));
        assert_eq!(saved.remark.as_deref(), Some("改过备注"));
    }
}

/* ── 隧道配置 ── */

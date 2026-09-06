//! SSH 插件 · 服务器配置与分组持久化（插件私有 ssh.db，PluginDb 骨架）
//! profile 只存 credentialRef 引用；秘密本体在公共 Vault，永不落本库。
//! SQL 与行映射写在 &Connection 层（便于内存库单测），PluginDb 仅承担打开/迁移/锁。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::framework::store::PluginDb;
use crate::plugins::ssh::models::{AuthMethod, ServerProfile, SshGroup};

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
    let conn = Connection::open_in_memory().expect("内存库打开失败");
    crate::framework::store::migrate(&conn, MIGRATIONS).expect("内存库迁移失败");
    conn
}

/* ── 分组 ── */

/// 分组列表（按 sort_order 排列）
pub(crate) fn list_groups(conn: &Connection) -> Result<Vec<SshGroup>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, sort_order FROM ssh_groups ORDER BY sort_order, id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SshGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// 新增/更新分组（同 id 覆盖名称与排序）
pub(crate) fn upsert_group(conn: &Connection, group: &SshGroup) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ssh_groups (id, name, sort_order) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, sort_order = excluded.sort_order",
        rusqlite::params![group.id, group.name, group.sort_order],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除分组；组内 profile 移回未分组（配置本身不删）
pub(crate) fn delete_group(conn: &Connection, group_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE ssh_profiles SET group_id = NULL WHERE group_id = ?1",
        [group_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM ssh_groups WHERE id = ?1", [group_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/* ── 服务器配置 ── */

/// 行 → ServerProfile（列名与 SELECT 清单一致）
fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<ServerProfile> {
    Ok(ServerProfile {
        id: row.get("id")?,
        name: row.get("name")?,
        host: row.get("host")?,
        port: row.get::<_, i64>("port")? as u16,
        username: row.get("username")?,
        auth_method: auth_method_from_str(&row.get::<_, String>("auth_method")?),
        credential_ref: row.get("credential_ref")?,
        group_id: row.get("group_id")?,
        remark: row.get("remark")?,
        last_connected_at: row.get("last_connected_at")?,
    })
}

/// 全部服务器配置（按创建顺序）
pub(crate) fn list_profiles(conn: &Connection) -> Result<Vec<ServerProfile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, username, auth_method, credential_ref, group_id,
             remark, last_connected_at FROM ssh_profiles ORDER BY created_at, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_profile)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// 按 id 取单个配置（不存在时报错）
pub(crate) fn get_profile(conn: &Connection, id: &str) -> Result<ServerProfile, String> {
    conn.query_row(
        "SELECT id, name, host, port, username, auth_method, credential_ref, group_id,
         remark, last_connected_at FROM ssh_profiles WHERE id = ?1",
        [id],
        row_to_profile,
    )
    .map_err(|_| format!("服务器配置不存在（{id}）"))
}

/// 新增/更新配置（credentialRef 随行保存；时间戳由调用方传入以便测试）
pub(crate) fn upsert_profile(
    conn: &Connection,
    profile: &ServerProfile,
    now_ms: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ssh_profiles (id, name, host, port, username, auth_method, credential_ref,
         group_id, remark, last_connected_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name, host = excluded.host, port = excluded.port,
           username = excluded.username, auth_method = excluded.auth_method,
           credential_ref = excluded.credential_ref, group_id = excluded.group_id,
           remark = excluded.remark, last_connected_at = excluded.last_connected_at,
           updated_at = excluded.updated_at",
        rusqlite::params![
            profile.id,
            profile.name,
            profile.host,
            profile.port,
            profile.username,
            auth_method_to_str(profile.auth_method),
            profile.credential_ref,
            profile.group_id,
            profile.remark,
            profile.last_connected_at,
            now_ms
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除配置（只删配置行；Vault 凭证由用户在凭证库自行管理）
pub(crate) fn delete_profile(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_profiles WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 记录最近连接时间（连接成功后调用）
pub(crate) fn touch_last_connected(conn: &Connection, id: &str, now_ms: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE ssh_profiles SET last_connected_at = ?2 WHERE id = ?1",
        rusqlite::params![id, now_ms],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 认证方式 → 存储字符串
pub(crate) fn auth_method_to_str(method: AuthMethod) -> &'static str {
    match method {
        AuthMethod::Password => "password",
        AuthMethod::PrivateKey => "privateKey",
        AuthMethod::PrivateKeyWithPassphrase => "privateKeyWithPassphrase",
    }
}

/// 存储字符串 → 认证方式（未知值兜底为密码认证）
pub(crate) fn auth_method_from_str(value: &str) -> AuthMethod {
    match value {
        "privateKey" => AuthMethod::PrivateKey,
        "privateKeyWithPassphrase" => AuthMethod::PrivateKeyWithPassphrase,
        _ => AuthMethod::Password,
    }
}

/* ── 命令层 ── */

use crate::framework::vault::models::{CredentialFields, CredentialSavePayload};
use crate::plugins::ssh::models::{
    SshImportResult, SshProfileImportPayload, SshProfileSavePayload,
};

/// 服务器配置列表
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_profile_list(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<Vec<ServerProfile>, String> {
    with_db(&app, &state, list_profiles)
}

/// 新增/更新服务器配置；勾选保存凭证时写入 Vault 并回填 credentialRef（同 id upsert，避免重复条目）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_profile_save(
    app: AppHandle,
    state: State<'_, ProfileState>,
    payload: SshProfileSavePayload,
) -> Result<ServerProfile, String> {
    let mut profile = payload.profile;
    if payload.save_credential {
        // 共享保护：该 credentialRef 还被其他服务器引用时，强制新建而非改名/覆盖原凭证
        let shared = profile
            .credential_ref
            .as_deref()
            .map(|rid| {
                with_db(&app, &state, |conn| {
                    credential_shared_by_others(conn, &profile.id, rid)
                })
            })
            .unwrap_or(Ok(false))
            .unwrap_or(false);
        if shared {
            profile.credential_ref = None;
        }
        let has_password = !payload.password.as_deref().unwrap_or_default().is_empty();
        let has_key = !payload
            .private_key
            .as_deref()
            .unwrap_or_default()
            .is_empty();
        if has_password || has_key {
            let fields = if profile.auth_method == AuthMethod::Password {
                CredentialFields::Password {
                    username: profile.username.clone(),
                    password: payload.password.clone().unwrap_or_default(),
                }
            } else {
                if profile.auth_method == AuthMethod::PrivateKeyWithPassphrase
                    && payload.passphrase.as_deref().unwrap_or_default().is_empty()
                {
                    return Err("该认证方式需要 Passphrase".into());
                }
                CredentialFields::SshKey {
                    username: profile.username.clone(),
                    private_key: payload.private_key.clone().unwrap_or_default(),
                    passphrase: payload.passphrase.clone(),
                }
            };
            // 已有引用时 upsert 同一条目（避免重复凭证）；引用失效（凭证已删）回退新建
            let build_payload = |id: Option<String>| CredentialSavePayload {
                id,
                name: format!("{}（SSH）", profile.name),
                kind: fields.kind(),
                fields: fields.clone(),
                note: String::new(),
            };
            let summary = match profile.credential_ref.clone() {
                Some(id) => {
                    // 已有引用时 upsert 同一条目（避免重复凭证）；失效（已删）回退新建
                    match crate::framework::vault::vault_save(app.clone(), build_payload(Some(id)))
                    {
                        Ok(summary) => summary,
                        Err(e) => {
                            if !e.contains("不存在") {
                                return Err(e);
                            }
                            crate::framework::vault::vault_save(app.clone(), build_payload(None))?
                        }
                    }
                }
                None => crate::framework::vault::vault_save(app.clone(), build_payload(None))?,
            };
            profile.credential_ref = Some(summary.id);
        }
    }
    let saved = profile.clone();
    with_db(&app, &state, |conn| {
        upsert_profile(conn, &saved, crate::plugins::ssh::conn::now_ms() as i64)
    })?;
    Ok(profile)
}

/// 删除服务器配置（只删配置行；Vault 凭证保留在凭证库由用户管理）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_profile_delete(
    app: AppHandle,
    state: State<'_, ProfileState>,
    profile_id: String,
) -> Result<(), String> {
    with_db(&app, &state, |conn| delete_profile(conn, &profile_id))
}

/// 分组列表
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_group_list(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<Vec<SshGroup>, String> {
    with_db(&app, &state, list_groups)
}

/// 新增/更新分组
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_group_save(
    app: AppHandle,
    state: State<'_, ProfileState>,
    group: SshGroup,
) -> Result<(), String> {
    with_db(&app, &state, |conn| upsert_group(conn, &group))
}

/// 删除分组（组内配置移回未分组）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_group_delete(
    app: AppHandle,
    state: State<'_, ProfileState>,
    group_id: String,
) -> Result<(), String> {
    with_db(&app, &state, |conn| delete_group(conn, &group_id))
}

/// localStorage 存量数据一次性导入：profiles/groups 入库，旧手工凭证（AES 文件）迁入 Vault。
/// 全程幂等（同 id upsert）；迁移成功后归档旧凭证文件（改名保留，不删除）。
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_profile_import(
    app: AppHandle,
    state: State<'_, ProfileState>,
    payload: SshProfileImportPayload,
) -> Result<SshImportResult, String> {
    // 旧 AES 凭证文件（profileId → {authMethod, password, privateKey, passphrase}）
    // 解密失败（主密钥丢失等）只跳过凭证迁移，配置照常导入——否则用户视角=服务器列表消失
    let mut legacy_credentials_failed = false;
    let legacy = match crate::plugins::ssh::credential::read_all(&app) {
        Ok(map) => map,
        Err(e) => {
            eprintln!("[ssh] 旧凭证读取失败，跳过凭证迁移: {e}");
            legacy_credentials_failed = true;
            HashMap::new()
        }
    };
    let now = crate::plugins::ssh::conn::now_ms() as i64;
    let mut migrated_credentials = 0usize;

    let profiles = payload.profiles.clone();
    let groups = payload.groups.clone();
    with_db(&app, &state, |conn| {
        for group in &groups {
            upsert_group(conn, group)?;
        }
        for profile in &profiles {
            let mut to_save = profile.clone();
            // 已有 Vault 引用（新数据）直接保留；否则查旧手工凭证并迁移
            let has_ref = to_save
                .credential_ref
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if !has_ref {
                to_save.credential_ref = None;
                if let Some(legacy) = legacy.get(&profile.id) {
                    let auth_method = auth_method_from_str(
                        legacy
                            .get("authMethod")
                            .and_then(|v| v.as_str())
                            .unwrap_or("password"),
                    );
                    let password = legacy
                        .get("password")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty());
                    let private_key = legacy
                        .get("privateKey")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty());
                    if password.is_some() || private_key.is_some() {
                        let fields = if auth_method == AuthMethod::Password {
                            CredentialFields::Password {
                                username: profile.username.clone(),
                                password: password.unwrap_or_default().to_string(),
                            }
                        } else {
                            CredentialFields::SshKey {
                                username: profile.username.clone(),
                                private_key: private_key.unwrap_or_default().to_string(),
                                passphrase: legacy
                                    .get("passphrase")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(String::from),
                            }
                        };
                        let summary = crate::framework::vault::vault_save(
                            app.clone(),
                            CredentialSavePayload {
                                id: None,
                                name: format!("{}（SSH）", profile.name),
                                kind: fields.kind(),
                                fields,
                                note: "由 SSH 手工凭证自动迁移".into(),
                            },
                        )?;
                        to_save.credential_ref = Some(summary.id);
                        migrated_credentials += 1;
                    }
                }
            }
            upsert_profile(conn, &to_save, now)?;
        }
        Ok(())
    })?;

    // 迁移成功后归档旧凭证文件（失败不影响导入结果，下次启动可重试归档）
    if migrated_credentials > 0 {
        if let Err(e) = crate::plugins::ssh::credential::archive_legacy_file(&app) {
            eprintln!("[ssh] 旧凭证文件归档失败（不影响迁移结果）: {e}");
        }
    }
    Ok(SshImportResult {
        imported_profiles: profiles.len(),
        imported_groups: groups.len(),
        migrated_credentials,
        legacy_credentials_failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::ssh::models::SshGroup;

    fn profile(id: &str, group_id: Option<&str>) -> ServerProfile {
        ServerProfile {
            id: id.into(),
            name: format!("服务器 {id}"),
            host: "10.0.0.5".into(),
            port: 22,
            username: "root".into(),
            auth_method: AuthMethod::Password,
            credential_ref: Some("cred-1".into()),
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
}

/* ── 隧道配置 ── */

/// 行 → TunnelConfig（列名与 SELECT 清单一致）
fn row_to_tunnel(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::plugins::ssh::models::TunnelConfig> {
    use crate::plugins::ssh::models::TunnelType;
    Ok(crate::plugins::ssh::models::TunnelConfig {
        id: row.get("id")?,
        profile_id: row.get("profile_id")?,
        name: row.get("name")?,
        tunnel_type: TunnelType::from_str(&row.get::<_, String>("tunnel_type")?),
        listen_host: row.get("listen_host")?,
        listen_port: row.get::<_, i64>("listen_port")? as u16,
        target_host: row.get("target_host")?,
        target_port: row.get::<_, Option<i64>>("target_port")?.map(|v| v as u16),
        auto_start: row.get::<_, i64>("auto_start")? != 0,
    })
}

/// 检查某 credentialRef 是否还被其他 profile 引用（共享保护用）
pub(crate) fn credential_shared_by_others(
    conn: &Connection,
    profile_id: &str,
    credential_ref: &str,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ssh_profiles WHERE credential_ref = ?1 AND id != ?2",
            rusqlite::params![credential_ref, profile_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

/// 某服务器的全部隧道配置（按创建顺序）
pub(crate) fn list_tunnels(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<crate::plugins::ssh::models::TunnelConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, name, tunnel_type, listen_host, listen_port, target_host, target_port, auto_start
             FROM ssh_tunnels WHERE profile_id = ?1 ORDER BY created_at, id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([profile_id], row_to_tunnel)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// 单条隧道配置
pub(crate) fn get_tunnel(
    conn: &Connection,
    id: &str,
) -> Result<crate::plugins::ssh::models::TunnelConfig, String> {
    conn.query_row(
        "SELECT id, profile_id, name, tunnel_type, listen_host, listen_port, target_host, target_port, auto_start
         FROM ssh_tunnels WHERE id = ?1",
        [id],
        row_to_tunnel,
    )
    .map_err(|_| format!("隧道配置不存在（{id}）"))
}

/// 新增/更新隧道配置
pub(crate) fn upsert_tunnel(
    conn: &Connection,
    config: &crate::plugins::ssh::models::TunnelConfig,
    now_ms: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ssh_tunnels (id, profile_id, name, tunnel_type, listen_host, listen_port, target_host, target_port, auto_start, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           profile_id = excluded.profile_id, name = excluded.name, tunnel_type = excluded.tunnel_type,
           listen_host = excluded.listen_host, listen_port = excluded.listen_port,
           target_host = excluded.target_host, target_port = excluded.target_port,
           auto_start = excluded.auto_start",
        rusqlite::params![
            config.id,
            config.profile_id,
            config.name,
            config.tunnel_type.as_str(),
            config.listen_host,
            config.listen_port,
            config.target_host,
            config.target_port,
            config.auto_start as i64,
            now_ms
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除隧道配置
pub(crate) fn delete_tunnel(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_tunnels WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 某服务器的隧道配置列表（命令）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_tunnel_list(
    app: AppHandle,
    state: State<'_, ProfileState>,
    profile_id: String,
) -> Result<Vec<crate::plugins::ssh::models::TunnelConfig>, String> {
    with_db(&app, &state, |conn| list_tunnels(conn, &profile_id))
}

/// 新增/更新隧道配置（命令）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_tunnel_save(
    app: AppHandle,
    state: State<'_, ProfileState>,
    config: crate::plugins::ssh::models::TunnelConfig,
) -> Result<crate::plugins::ssh::models::TunnelConfig, String> {
    with_db(&app, &state, |conn| {
        upsert_tunnel(conn, &config, crate::plugins::ssh::conn::now_ms() as i64)
    })?;
    Ok(config)
}

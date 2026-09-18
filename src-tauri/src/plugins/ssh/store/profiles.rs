//! SSH 插件 · 服务器配置与分组持久化（ssh.db；profile 只存 credentialRef 引用）

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::plugins::ssh::models::{AuthMethod, ServerProfile, SshGroup};
use crate::plugins::ssh::store::{with_db, ProfileState};

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

/// 清空全部分组（覆盖导入用；调用方负责在同一事务内重建）
pub(crate) fn clear_groups(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_groups", [])
        .map_err(|e| format!("清空分组失败: {e}"))?;
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
           remark = excluded.remark,
           /* 编辑保存可能不带回最近连接时间（serde 缺省字段缺席即 None）：
              excluded 为 NULL 时保留库里的原值，不能抹掉 */
           last_connected_at = COALESCE(excluded.last_connected_at, ssh_profiles.last_connected_at),
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

/// 清空全部档案（覆盖导入用；调用方负责在同一事务内重建）
pub(crate) fn clear_profiles(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_profiles", [])
        .map_err(|e| format!("清空档案失败: {e}"))?;
    Ok(())
}

/// 删除配置并级联清理关联数据（目录书签、隧道配置——两表无外键，不显式删就成孤儿行）；
/// Vault 凭证由用户在凭证库自行管理，不在此删。
pub(crate) fn delete_profile(conn: &Connection, id: &str) -> Result<(), String> {
    // unchecked_transaction：rusqlite 事务 API 要 &mut Connection，本层统一 &Connection，
    // 用其拿事务句柄保证三删原子（PluginDb 串行化连接访问，无并发嵌套事务）
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM profile_bookmarks WHERE profile_id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM ssh_tunnels WHERE profile_id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM ssh_profiles WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
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

use super::tunnels::credential_shared_by_others;
use crate::plugins::ssh::models::SshProfileSavePayload;

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

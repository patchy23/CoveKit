//! SSH 插件 · 隧道配置持久化（ssh_tunnels 表）

use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::plugins::ssh::store::{with_db, ProfileState};

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

/// 清空全部隧道（覆盖导入用；调用方负责在同一事务内重建）
pub(crate) fn clear_tunnels(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM ssh_tunnels", [])
        .map_err(|e| format!("清空隧道失败: {e}"))?;
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

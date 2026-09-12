//! FRP 客户端登记（frpc 可执行文件的引用清单）
//!
//! 设计取舍：**只登记路径，不复制文件**。定制客户端常由用户自己编译在固定目录里持续更新，
//! 复制进工具目录会立刻过期；工具内下载的版本按版本号分文件落在下载目录，两者同表统一管理。
//! 表结构在 `mod.rs` 的 `MIGRATIONS`（v2 追加），本模块只负责读写与解析。

use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::framework::store::PluginDb;
use crate::plugins::frp::binary;
use crate::plugins::frp::models::{FrpClient, FrpClientList, FrpClientSource};
use crate::plugins::frp::{now_ms, MIGRATIONS, TOOL_ID};

/// 数据库中的一条客户端记录（不含「文件是否还在」这类运行时探测结果）
#[derive(Debug, Clone)]
struct ClientRow {
    /// 稳定标识（路径哈希）
    id: String,
    /// 展示名
    label: String,
    /// 可执行文件绝对路径
    path: String,
    /// 缓存的版本号（空串表示尚未探测成功）
    version: String,
    /// 来源：工具内下载 / 外部引用
    source: FrpClientSource,
    /// 是否为默认客户端
    is_default: bool,
}

/// 路径 → 稳定标识（同路径重复登记幂等；哈希避免绝对路径出现在主键与日志里）
pub(crate) fn client_id(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    // 取前 8 字节十六进制：区分度足够，且短到能直接放进界面与日志
    hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 展示名：文件名去掉扩展名（定制客户端的文件名通常已能说明身份）
fn label_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(String::from)
        .unwrap_or_else(|| "frpc".to_string())
}

/// 来源 → 数据库字面量
fn source_text(source: FrpClientSource) -> &'static str {
    match source {
        FrpClientSource::Download => "download",
        FrpClientSource::External => "external",
    }
}

/// 数据库字面量 → 来源（未知值按外部引用处理，保证旧数据可读）
fn source_from_text(text: &str) -> FrpClientSource {
    match text {
        "download" => FrpClientSource::Download,
        _ => FrpClientSource::External,
    }
}

/// 读取全部客户端记录（无库或读失败时返回空表，不阻断列表展示）
fn read_rows(app: &AppHandle) -> Vec<ClientRow> {
    let Ok(db) = PluginDb::open(app, TOOL_ID, MIGRATIONS) else {
        return Vec::new();
    };
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, label, path, version, source, is_default FROM clients")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ClientRow {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    path: row.get(2)?,
                    version: row.get(3)?,
                    source: source_from_text(&row.get::<_, String>(4)?),
                    is_default: row.get::<_, i64>(5)? != 0,
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect::<Vec<ClientRow>>())
    })
    .unwrap_or_default()
}

/// 写入一条记录（upsert；不覆盖既有默认标记，默认项只由 set_default 改动）
fn upsert_row(app: &AppHandle, row: &ClientRow) -> Result<(), String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO clients (id, label, path, version, source, is_default, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                label = excluded.label,
                path = excluded.path,
                version = excluded.version,
                source = excluded.source,
                last_seen = excluded.last_seen",
            rusqlite::params![
                row.id,
                row.label,
                row.path,
                row.version,
                source_text(row.source),
                i64::from(row.is_default),
                now_ms(),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 回写探测到的版本号（列表懒探测后缓存，避免每次列表都跑一次进程）
fn cache_version(app: &AppHandle, id: &str, version: &str) -> Result<(), String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE clients SET version = ?2, last_seen = ?3 WHERE id = ?1",
            rusqlite::params![id, version, now_ms()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 把某个客户端设为唯一的默认项（先清空全部标记再置位，保证全局唯一）
pub(crate) fn set_default(app: &AppHandle, id: &str) -> Result<(), String> {
    let known = read_rows(app).iter().any(|row| row.id == id);
    if !known {
        return Err("客户端不存在（可能已被移除）".to_string());
    }
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE clients SET is_default = 0 WHERE is_default <> 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE clients SET is_default = 1 WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 移除客户端登记。**只删记录不删文件**：外部引用的文件不属于本工具，
/// 工具内下载的文件也保留，避免「删个记录顺手删掉用户东西」。
pub(crate) fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let was_default = read_rows(app)
        .iter()
        .any(|row| row.id == id && row.is_default);
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| {
        conn.execute("DELETE FROM clients WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        // 解绑所有指向它的档案，否则档案会拿着一个不存在的 id 无法启动
        conn.execute(
            "DELETE FROM profile_client WHERE client_id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })?;
    // 默认项被移除后，顺位把剩下的第一个顶上，避免出现「有客户端但没默认」的空档
    if was_default {
        if let Some(next) = read_rows(app).first() {
            set_default(app, &next.id)?;
        }
    }
    Ok(())
}

/// 登记一个客户端（同路径幂等；先写库再探测版本，探测失败不影响登记）
async fn register(
    app: &AppHandle,
    path: &Path,
    source: FrpClientSource,
) -> Result<FrpClient, String> {
    if !path.is_file() {
        return Err(format!("不是可执行文件：{}", path.display()));
    }
    let id = client_id(path);
    let existing = read_rows(app).into_iter().find(|row| row.id == id);
    let is_default = existing
        .as_ref()
        .map(|row| row.is_default)
        .unwrap_or_else(|| read_rows(app).is_empty());
    let version = binary::probe_version(path).await.unwrap_or_default();
    let row = ClientRow {
        id,
        label: label_of(path),
        path: path.display().to_string(),
        version,
        source,
        is_default,
    };
    upsert_row(app, &row)?;
    Ok(to_client(&row))
}

/// 记录行 → 前端结构（补「文件是否还在」的实时判断）
fn to_client(row: &ClientRow) -> FrpClient {
    FrpClient {
        id: row.id.clone(),
        label: row.label.clone(),
        path: row.path.clone(),
        version: if row.version.is_empty() {
            None
        } else {
            Some(row.version.clone())
        },
        source: row.source,
        is_default: row.is_default,
        exists: Path::new(&row.path).is_file(),
    }
}

/// 登记一个外部引用的可执行文件（不复制文件）
pub(crate) async fn add_external(app: &AppHandle, raw_path: &str) -> Result<FrpClient, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("请选择 frpc 可执行文件".to_string());
    }
    let path = PathBuf::from(trimmed);
    let absolute = std::fs::canonicalize(&path)
        .map_err(|e| format!("无法解析路径（{}）：{e}", path.display()))?;
    register(app, &absolute, FrpClientSource::External).await
}

/// 登记工具内下载完成的客户端（下载命令收尾时调用）
pub(crate) async fn add_downloaded(app: &AppHandle, path: &Path) -> Result<FrpClient, String> {
    register(app, path, FrpClientSource::Download).await
}

/// 是否存在任何已登记的客户端
pub(crate) fn has_any(app: &AppHandle) -> bool {
    !read_rows(app).is_empty()
}

/// 首次使用时把当前能探测到的 frpc 自动补登记为默认客户端。
/// 这样既有用户（设置里填过 frpcPath、或用旧版一键下载装过 frpc.exe）无需手动添加，
/// 也不会因为引入清单机制而「突然找不到客户端」。
pub(crate) async fn seed_if_empty(app: &AppHandle) {
    if has_any(app) {
        return;
    }
    let detected = binary::detect(app).await;
    let Some(raw) = detected.path else {
        return;
    };
    // 探测结果可能来自 PATH 或常见位置，统一按外部引用登记（不搬动文件）
    let source = match detected.source {
        Some(crate::plugins::frp::models::FrpBinarySource::Downloaded) => FrpClientSource::Download,
        _ => FrpClientSource::External,
    };
    let path = PathBuf::from(&raw);
    // 首个登记项在 register 内部已自动置为默认，这里无需再补设置
    let _ = register(app, &path, source).await;
}

/// 档案绑定的客户端 id（None = 跟随默认）
pub(crate) fn binding(app: &AppHandle, file_name: &str) -> Option<String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS).ok()?;
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT client_id FROM profile_client WHERE file_name = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(rusqlite::params![file_name], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        Ok(rows.next().and_then(|row| row.ok()))
    })
    .ok()
    .flatten()
}

/// 设置档案绑定的客户端（None 表示解除绑定、回到跟随默认）
pub(crate) fn bind(
    app: &AppHandle,
    file_name: &str,
    client_id: Option<&str>,
) -> Result<(), String> {
    let db = PluginDb::open(app, TOOL_ID, MIGRATIONS)?;
    db.with_conn(|conn| match client_id {
        Some(id) => {
            conn.execute(
                "INSERT INTO profile_client (file_name, client_id) VALUES (?1, ?2)
                 ON CONFLICT(file_name) DO UPDATE SET client_id = excluded.client_id",
                rusqlite::params![file_name, id],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        None => {
            conn.execute(
                "DELETE FROM profile_client WHERE file_name = ?1",
                rusqlite::params![file_name],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
    })
}

/// 列出客户端清单：懒探测未记录版本的项，并标记文件是否仍存在
pub(crate) async fn list(app: &AppHandle) -> FrpClientList {
    seed_if_empty(app).await;
    let rows = read_rows(app);
    let mut clients = Vec::with_capacity(rows.len());
    for row in rows {
        let mut client = to_client(&row);
        // 版本只在「文件在、但没记录过版本」时补探测一次，结果写回缓存
        if client.exists && client.version.is_none() {
            if let Some(version) = binary::probe_version(Path::new(&row.path)).await {
                let _ = cache_version(app, &row.id, &version);
                client.version = Some(version);
            }
        }
        clients.push(client);
    }
    let default_id = clients
        .iter()
        .find(|client| client.is_default)
        .map(|client| client.id.clone());
    FrpClientList {
        ok: true,
        clients,
        default_id,
        error: None,
    }
}

/// 解析某个档案实际要用的 frpc 可执行文件。
/// 优先级：档案绑定 → 默认客户端 → 兜底自动探测（保证「清单为空也能跑」）。
pub(crate) async fn resolve(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    seed_if_empty(app).await;
    let rows = read_rows(app);
    if let Some(bound) = binding(app, file_name) {
        match rows.iter().find(|row| row.id == bound) {
            Some(row) => {
                let path = PathBuf::from(&row.path);
                if path.is_file() {
                    return Ok(path);
                }
                // 绑定项的文件没了：明确报错而不是静默换一个版本——
                // 服务端有版本限制时，悄悄换客户端比启动失败更难排查
                return Err(format!(
                    "档案绑定的客户端已不存在：{}（请在客户端管理里重新选择或指定）",
                    row.path
                ));
            }
            None => {
                return Err("档案绑定的客户端已被移除，请在客户端管理中重新指定".to_string());
            }
        }
    }
    if let Some(row) = rows.iter().find(|row| row.is_default) {
        let path = PathBuf::from(&row.path);
        if path.is_file() {
            return Ok(path);
        }
    }
    let detected = binary::detect(app).await;
    detected.path.map(PathBuf::from).ok_or_else(|| {
        detected
            .error
            .unwrap_or_else(|| "未找到 frpc，请在客户端管理中添加或下载".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一路径必须得到同一个 id：重复登记幂等依赖这一点
    #[test]
    fn client_id_is_stable_for_same_path() {
        let first = client_id(Path::new("C:/tools/frpc.exe"));
        let second = client_id(Path::new("C:/tools/frpc.exe"));
        assert_eq!(first, second);
        assert_eq!(first.len(), 16, "id 为 8 字节十六进制");
    }

    /// 不同路径必须区分开，否则两个客户端会互相覆盖记录
    #[test]
    fn client_id_differs_for_different_paths() {
        let a = client_id(Path::new("C:/tools/frpc.exe"));
        let b = client_id(Path::new("D:/build/frpc-patched.exe"));
        assert_ne!(a, b);
    }

    /// 展示名取文件名去掉扩展名（用户靠文件名辨识定制客户端）
    #[test]
    fn label_uses_file_stem() {
        assert_eq!(
            label_of(Path::new("C:/tools/frpc-0.71.0.exe")),
            "frpc-0.71.0"
        );
        assert_eq!(label_of(Path::new("/usr/local/bin/frpc")), "frpc");
    }

    /// 来源字面量往返一致；未知值按外部引用兜底（保证旧数据可读）
    #[test]
    fn source_text_roundtrip() {
        assert_eq!(source_text(FrpClientSource::Download), "download");
        assert_eq!(source_text(FrpClientSource::External), "external");
        assert_eq!(source_from_text("download"), FrpClientSource::Download);
        assert_eq!(source_from_text("external"), FrpClientSource::External);
        assert_eq!(source_from_text("unknown"), FrpClientSource::External);
    }
}

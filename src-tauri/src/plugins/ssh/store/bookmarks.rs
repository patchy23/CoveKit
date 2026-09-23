//! SSH 插件 · 目录书签持久化（profile_bookmarks 表；按服务器 profile 隔离，全会话共享）

use rusqlite::Connection;
use tauri::{AppHandle, State};

use super::{with_db, ProfileState};
use crate::plugins::ssh::models::SshBookmark;

/// 列出某服务器的书签（按 sort 升序）
pub fn list_bookmarks(conn: &Connection, profile_id: &str) -> Result<Vec<SshBookmark>, String> {
    let mut stmt = conn
        .prepare("SELECT id, profile_id, name, path, sort FROM profile_bookmarks WHERE profile_id = ?1 ORDER BY sort ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([profile_id], |row| {
            Ok(SshBookmark {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                sort: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 新增书签（sort 取当前最大值 +1，追加到末尾；同路径重复时直接返回已有条目）
pub fn add_bookmark(
    conn: &Connection,
    profile_id: &str,
    name: &str,
    path: &str,
) -> Result<SshBookmark, String> {
    if !path.starts_with('/') {
        return Err("书签必须是远程绝对路径".into());
    }
    let path = if path.trim_end_matches('/').is_empty() {
        "/"
    } else {
        path.trim_end_matches('/')
    };
    // 同路径去重：已存在直接返回（右键重复添加不应产生重复行）
    if let Some(existing) = list_bookmarks(conn, profile_id)?
        .into_iter()
        .find(|b| b.path == path)
    {
        return Ok(existing);
    }
    let max_sort: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort), 0) FROM profile_bookmarks WHERE profile_id = ?1",
            [profile_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let bookmark = SshBookmark {
        id: crate::plugins::ssh::conn::resource_id("bm"),
        profile_id: profile_id.into(),
        name: name.into(),
        path: path.into(),
        sort: max_sort + 1,
    };
    conn.execute(
        "INSERT INTO profile_bookmarks (id, profile_id, name, path, sort) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![bookmark.id, bookmark.profile_id, bookmark.name, bookmark.path, bookmark.sort],
    )
    .map_err(|e| e.to_string())?;
    Ok(bookmark)
}

/// 清空全部书签（覆盖导入用；调用方负责在同一事务内重建）
pub(crate) fn clear_bookmarks(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM profile_bookmarks", [])
        .map_err(|e| format!("清空书签失败: {e}"))?;
    Ok(())
}

/// 删除书签
pub fn delete_bookmark(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM profile_bookmarks WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 按主键写入书签（数据导入用：保留原有 id 与顺序）
///
/// 为什么不复用 `add_bookmark`：那条路径是给交互用的，会按路径去重并生成新 id，
/// 导入必须原样保留传输标识，否则同一份包导入两次会得到不同的书签。
pub fn upsert_bookmark(conn: &Connection, bookmark: &SshBookmark) -> Result<(), String> {
    conn.execute(
        "INSERT INTO profile_bookmarks (id, profile_id, name, path, sort) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET profile_id = excluded.profile_id, name = excluded.name,
           path = excluded.path, sort = excluded.sort",
        rusqlite::params![
            bookmark.id,
            bookmark.profile_id,
            bookmark.name,
            bookmark.path,
            bookmark.sort
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 书签列表（命令入口）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_bookmark_list(
    app: AppHandle,
    state: State<'_, ProfileState>,
    profile_id: String,
) -> Result<Vec<SshBookmark>, String> {
    with_db(&app, &state, |conn| list_bookmarks(conn, &profile_id))
}

/// 新增书签（命令入口）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_bookmark_add(
    app: AppHandle,
    state: State<'_, ProfileState>,
    profile_id: String,
    name: String,
    path: String,
) -> Result<SshBookmark, String> {
    with_db(&app, &state, |conn| {
        add_bookmark(conn, &profile_id, &name, &path)
    })
}

/// 删除书签（命令入口）
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_bookmark_delete(
    app: AppHandle,
    state: State<'_, ProfileState>,
    id: String,
) -> Result<(), String> {
    with_db(&app, &state, |conn| delete_bookmark(conn, &id))
}

/// 更新当前服务器的书签名称和排序；不允许通过此入口变更归属或路径。
#[tauri::command(rename_all = "camelCase")]
pub fn ssh_bookmark_update(
    app: AppHandle,
    state: State<'_, ProfileState>,
    profile_id: String,
    bookmarks: Vec<SshBookmark>,
) -> Result<(), String> {
    with_db(&app, &state, |conn| {
        update_bookmarks(conn, &profile_id, &bookmarks)
    })
}

/// 整组重排在同一事务中完成，任何非法项都回滚此前更新。
fn update_bookmarks(
    conn: &Connection,
    profile_id: &str,
    bookmarks: &[SshBookmark],
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let existing = list_bookmarks(&tx, profile_id)?;
    let ids: std::collections::HashSet<_> = bookmarks.iter().map(|b| &b.id).collect();
    if bookmarks.len() != existing.len() || ids.len() != existing.len() {
        return Err("书签已变化，请刷新后重试".into());
    }
    for (index, bookmark) in bookmarks.iter().enumerate() {
        if bookmark.name.trim().is_empty()
            || !existing.iter().any(|item| {
                item.id == bookmark.id
                    && item.path == bookmark.path
                    && bookmark.profile_id == profile_id
            })
        {
            return Err("书签无效或已变化，请刷新后重试".into());
        }
        tx.execute(
            "UPDATE profile_bookmarks SET name = ?1, sort = ?2 WHERE id = ?3 AND profile_id = ?4",
            rusqlite::params![bookmark.name.trim(), index as i64, bookmark.id, profile_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::store::migrate;

    fn open_memory() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn, super::super::MIGRATIONS).expect("内存库迁移失败");
        conn
    }

    #[test]
    fn 书签增删查与同路径去重() {
        let conn = open_memory();
        let a = add_bookmark(&conn, "p1", "日志", "/var/log").unwrap();
        let b = add_bookmark(&conn, "p1", "日志", "/var/log").unwrap();
        assert_eq!(a.id, b.id, "同路径重复添加应返回已有条目");
        add_bookmark(&conn, "p1", "部署", "/home/app").unwrap();
        let list = list_bookmarks(&conn, "p1").unwrap();
        assert_eq!(list.len(), 2);
        // 按 profile 隔离
        assert!(list_bookmarks(&conn, "p2").unwrap().is_empty());
        delete_bookmark(&conn, &a.id).unwrap();
        assert_eq!(list_bookmarks(&conn, "p1").unwrap().len(), 1);
    }
    #[test]
    fn reorder_is_atomic_and_checks_ownership() {
        let db = open_memory();
        add_bookmark(&db, "p", "one", "/one").unwrap();
        add_bookmark(&db, "p", "two", "/two").unwrap();
        let mut rows = list_bookmarks(&db, "p").unwrap();
        rows.reverse();
        rows[0].name = "renamed".into();
        update_bookmarks(&db, "p", &rows).unwrap();
        assert_eq!(list_bookmarks(&db, "p").unwrap()[0].name, "renamed");
        rows[0].name = "must rollback".into();
        rows[1].profile_id = "other".into();
        assert!(update_bookmarks(&db, "p", &rows).is_err());
        assert_eq!(list_bookmarks(&db, "p").unwrap()[0].name, "renamed");
    }
}

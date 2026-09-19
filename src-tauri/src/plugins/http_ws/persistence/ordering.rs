//! 接口树移动与排序：一个事务同时更新归属和同级顺序，不改请求内容。
use super::{db, group_move, ApiState};
use rusqlite::{params, Connection};
use tauri::{AppHandle, State};

/// kind 为 api/group；anchor 是目标同级节点的身份，缺省时追加到末尾。
#[tauri::command]
pub fn api_tree_move(
    app: AppHandle,
    state: State<'_, ApiState>,
    kind: String,
    id: String,
    parent: String,
    anchor: Option<String>,
    after: bool,
) -> Result<String, String> {
    let guard = db(&app, &state)?;
    guard
        .as_ref()
        .ok_or("本地库未初始化")?
        .with_transaction(|conn| move_ordered(conn, &kind, &id, &parent, anchor.as_deref(), after))
}

fn move_ordered(
    conn: &Connection,
    kind: &str,
    id: &str,
    parent: &str,
    anchor: Option<&str>,
    after: bool,
) -> Result<String, String> {
    if !parent.is_empty() {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM api_groups WHERE path=?1)",
                [parent],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            return Err("目标分组不存在".into());
        }
    }
    let (table, key, destination) = match kind {
        "group" => (
            "api_groups",
            "path",
            group_move::move_tree(conn, id, parent)?,
        ),
        "api" => {
            let changed = conn
                .execute(
                    "UPDATE api_list SET group_name=?2 WHERE id=?1",
                    params![id, parent],
                )
                .map_err(|e| e.to_string())?;
            if changed != 1 {
                return Err("接口不存在".into());
            }
            ("api_list", "id", id.to_string())
        }
        _ => return Err("不支持的节点类型".into()),
    };
    let sql = if kind == "group" {
        "SELECT path, path FROM api_groups ORDER BY sort_order, path"
    } else {
        "SELECT CAST(id AS TEXT), group_name FROM api_list ORDER BY sort_order, id"
    };
    let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut siblings = Vec::new();
    for row in rows {
        let (node, path) = row.map_err(|e| e.to_string())?;
        let node_parent = if kind == "group" {
            path.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
        } else {
            &path
        };
        if node_parent == parent && node != destination {
            siblings.push(node);
        }
    }
    let index = match anchor {
        Some(anchor) => {
            siblings
                .iter()
                .position(|node| node == anchor)
                .ok_or("排序目标已变化，请刷新后重试")?
                + usize::from(after)
        }
        None => siblings.len(),
    };
    siblings.insert(index, destination.clone());
    // table/key 仅来自上方固定分支，不接收外部 SQL 标识符。
    let update = format!("UPDATE {table} SET sort_order=?2 WHERE {key}=?1");
    for (index, node) in siblings.iter().enumerate() {
        conn.execute(&update, params![node, index as i64])
            .map_err(|e| e.to_string())?;
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_move_is_atomic_and_preserves_request_content() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE api_groups(path TEXT PRIMARY KEY, sort_order INTEGER DEFAULT 0);
            CREATE TABLE api_list(id INTEGER PRIMARY KEY, group_name TEXT, body TEXT, updated_at TEXT, sort_order INTEGER DEFAULT 0);
            INSERT INTO api_groups(path) VALUES('a'),('b');
            INSERT INTO api_list VALUES(1,'a','body-1','date',0),(2,'b','body-2','date',0),(3,'b','body-3','date',1);").unwrap();
        {
            let tx = conn.transaction().unwrap();
            assert!(move_ordered(&tx, "api", "1", "b", Some("missing"), false).is_err());
        }
        assert_eq!(
            conn.query_row("SELECT group_name FROM api_list WHERE id=1", [], |r| r
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "a"
        );
        let tx = conn.transaction().unwrap();
        move_ordered(&tx, "api", "1", "b", Some("3"), false).unwrap();
        tx.commit().unwrap();
        let ids = conn
            .prepare("SELECT id FROM api_list WHERE group_name='b' ORDER BY sort_order")
            .unwrap()
            .query_map([], |r| r.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ids, vec![2, 1, 3]);
        assert_eq!(
            conn.query_row("SELECT body FROM api_list WHERE id=1", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "body-1"
        );
    }
}

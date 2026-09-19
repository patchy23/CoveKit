//! 侧栏排序只更新位置及分组，不经过认证配置保存链路。
use super::{with_db, ProfileState};
use rusqlite::{params, Connection};
use tauri::{AppHandle, State};

/// 原子更新一层分组或服务器排序；服务器可同时改变归属。
#[tauri::command]
pub fn ssh_tree_move(
    app: AppHandle,
    state: State<'_, ProfileState>,
    kind: String,
    id: String,
    parent: Option<String>,
    anchor: Option<String>,
    after: bool,
) -> Result<(), String> {
    with_db(&app, &state, |conn| {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        move_ordered(&tx, &kind, &id, parent.as_deref(), anchor.as_deref(), after)?;
        tx.commit().map_err(|e| e.to_string())
    })
}

fn move_ordered(
    conn: &Connection,
    kind: &str,
    id: &str,
    parent: Option<&str>,
    anchor: Option<&str>,
    after: bool,
) -> Result<(), String> {
    let (table, sql) = match kind {
        "group" if parent.is_none() => (
            "ssh_groups",
            "SELECT id FROM ssh_groups ORDER BY sort_order, id",
        ),
        "profile" => {
            if let Some(group) = parent {
                let exists: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM ssh_groups WHERE id=?1)",
                        [group],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if !exists {
                    return Err("目标分组不存在".into());
                }
            }
            let changed = conn
                .execute(
                    "UPDATE ssh_profiles SET group_id=?2 WHERE id=?1",
                    params![id, parent],
                )
                .map_err(|e| e.to_string())?;
            if changed != 1 {
                return Err("服务器不存在".into());
            }
            ("ssh_profiles", "SELECT id FROM ssh_profiles WHERE group_id IS ?1 ORDER BY sort_order, created_at, id")
        }
        _ => return Err("SSH 分组只支持一层结构".into()),
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows = if kind == "group" {
        stmt.query([])
    } else {
        stmt.query([parent])
    }
    .map_err(|e| e.to_string())?;
    let mut siblings = Vec::<String>::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        siblings.push(row.get(0).map_err(|e| e.to_string())?);
    }
    if !siblings.iter().any(|node| node == id) {
        return Err("移动对象不存在".into());
    }
    siblings.retain(|node| node != id);
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
    siblings.insert(index, id.to_string());
    let sql = format!("UPDATE {table} SET sort_order=?2 WHERE id=?1");
    for (index, node) in siblings.iter().enumerate() {
        conn.execute(&sql, params![node, index as i64])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_anchor_without_moving_profile() {
        let mut conn = super::super::open_memory();
        conn.execute_batch("INSERT INTO ssh_groups VALUES('g','group',0);
          INSERT INTO ssh_profiles(id,name,host,port,username,auth_method,created_at,updated_at) VALUES('p','name','host',22,'user','password',0,0);").unwrap();
        {
            let tx = conn.transaction().unwrap();
            assert!(move_ordered(&tx, "profile", "p", Some("g"), Some("missing"), false).is_err());
        }
        let parent: Option<String> = conn
            .query_row("SELECT group_id FROM ssh_profiles WHERE id='p'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(parent, None);
        let tx = conn.transaction().unwrap();
        move_ordered(&tx, "profile", "p", Some("g"), None, false).unwrap();
        tx.commit().unwrap();
        let parent: String = conn
            .query_row("SELECT group_id FROM ssh_profiles WHERE id='p'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(parent, "g");
    }
}

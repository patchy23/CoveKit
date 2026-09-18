//! 分组树移动：精确匹配路径边界，同名目标拒绝合并，所有写入由调用方事务包裹。
use super::ensure_group_paths;
use rusqlite::Connection;

fn exists(conn: &Connection, path: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM api_groups
         WHERE path=?1 OR substr(path,1,length(?1)+1)=?1||'/')",
        [path],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// 仅从 PluginDb 的事务闭包调用；任一步失败必须回滚目录及接口归属。
pub(super) fn move_tree(conn: &Connection, path: &str, parent: &str) -> Result<String, String> {
    if path.is_empty() || !exists(conn, path)? {
        return Err("源分组不存在，请刷新后重试".into());
    }
    if parent == path || parent.starts_with(&format!("{path}/")) {
        return Err("不能将分组移入自身或子分组".into());
    }
    if !parent.is_empty() && !exists(conn, parent)? {
        return Err("目标分组不存在，请刷新后重试".into());
    }
    let name = path.rsplit('/').next().ok_or("分组路径无效")?;
    if name.is_empty() {
        return Err("分组路径无效".into());
    }
    let destination = if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    };
    if destination == path {
        return Ok(destination);
    }
    if exists(conn, &destination)? {
        return Err("目标下已有同名分组，请选择其他分组".into());
    }
    // 旧库可能只记录叶子路径；移走后仍保留原上级空分组。
    if let Some((old_parent, _)) = path.rsplit_once('/') {
        ensure_group_paths(conn, old_parent)?;
    }
    conn.execute(
        "UPDATE api_groups SET path=?2||substr(path,length(?1)+1)
         WHERE path=?1 OR substr(path,1,length(?1)+1)=?1||'/'",
        rusqlite::params![path, destination],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE api_list SET group_name=?2||substr(group_name,length(?1)+1),
         updated_at=datetime('now','localtime')
         WHERE group_name=?1 OR substr(group_name,1,length(?1)+1)=?1||'/'",
        rusqlite::params![path, destination],
    )
    .map_err(|e| e.to_string())?;
    ensure_group_paths(conn, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE api_groups(path TEXT PRIMARY KEY);
            CREATE TABLE api_list(id INTEGER PRIMARY KEY, group_name TEXT, body TEXT, updated_at TEXT);
            INSERT INTO api_groups VALUES ('开发/用户/空组'), ('开发/用户2'), ('目标');
            INSERT INTO api_list VALUES (1,'开发/用户','原始内容',''),
                (2,'开发/用户/空组','子接口',''), (3,'开发/用户2','相似前缀','');").unwrap();
        conn
    }
    #[test]
    fn moves_subtree_and_preserves_identity_content_and_old_parent() {
        let mut conn = database();
        let tx = conn.transaction().unwrap();
        assert_eq!(move_tree(&tx, "开发/用户", "目标").unwrap(), "目标/用户");
        tx.commit().unwrap();
        assert!(exists(&conn, "开发").unwrap());
        assert!(!exists(&conn, "开发/用户").unwrap());
        assert!(exists(&conn, "目标/用户/空组").unwrap());
        let rows: Vec<(i64, String, String)> = conn
            .prepare("SELECT id,group_name,body FROM api_list ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                (1, "目标/用户".into(), "原始内容".into()),
                (2, "目标/用户/空组".into(), "子接口".into()),
                (3, "开发/用户2".into(), "相似前缀".into())
            ]
        );
        let tx = conn.transaction().unwrap();
        assert_eq!(move_tree(&tx, "目标/用户", "").unwrap(), "用户");
        tx.commit().unwrap();
        assert!(exists(&conn, "用户/空组").unwrap());
    }
    #[test]
    fn rejects_cycles_collisions_missing_targets_and_rolls_back_write_failure() {
        let mut conn = database();
        ensure_group_paths(&conn, "目标/用户").unwrap();
        for parent in ["开发/用户", "开发/用户/空组", "目标", "不存在"] {
            let tx = conn.transaction().unwrap();
            assert!(move_tree(&tx, "开发/用户", parent).is_err());
        }
        conn.execute_batch("CREATE TRIGGER reject_move BEFORE UPDATE ON api_list BEGIN SELECT RAISE(ABORT,'模拟写入失败'); END;").unwrap();
        {
            let tx = conn.transaction().unwrap();
            assert!(move_tree(&tx, "开发/用户", "").is_err());
        }
        assert!(exists(&conn, "开发/用户/空组").unwrap());
        assert!(!exists(&conn, "用户").unwrap());
    }
}

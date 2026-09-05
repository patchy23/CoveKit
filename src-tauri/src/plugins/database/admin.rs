//! 管理操作命令（建库/授权/表 DDL/索引/表维护）
//! 与 catalog.rs（元数据查询）分离：这里的命令会改动数据库（DDL/DCL），
//! SQL 一律由方言层构造（标识符引用 + 权限白名单），前端只传选项不拼 SQL。
//! 分步操作（建库+授权）返回逐步结果，前端据此打勾/标错，不静默吞错。

use mysql_async::prelude::Queryable;
use tauri::State;

use crate::plugins::database::dialect::dialect_for;
use crate::plugins::database::drivers::{DbSession, DbSessionEntry, DbState};
use crate::plugins::database::models::{
    DbCharsetOptions, DbGrantInput, DbIndexInfo, DbStepResult, DbUserInfo,
};

/// 取会话条目（连接不存在报错）
fn session(state: &State<'_, DbState>, conn_id: &str) -> Result<DbSessionEntry, String> {
    state.entry(conn_id)
}

/// 在会话上执行一条无结果集语句（管理 DDL/DCL 用），返回影响行数
async fn exec_admin(entry: &DbSessionEntry, sql: &str) -> Result<u64, String> {
    match &entry.session {
        DbSession::Mysql(pool) => {
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?;
            conn.query_drop(sql)
                .await
                .map_err(|e| format!("执行失败: {e}"))?;
            Ok(conn.affected_rows())
        }
        DbSession::Postgres(pool) => {
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            client
                .execute(sql, &[])
                .await
                .map_err(|e| format!("执行失败: {e}"))
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            guard
                .execute(sql, [])
                .map(|n| n as u64)
                .map_err(|e| format!("执行失败: {e}"))
        }
        DbSession::Redis(_) => Err("Redis 不支持该操作".to_string()),
        DbSession::Agent { .. } => Err("该类型暂不支持在线管理操作".to_string()),
    }
}

/// 在会话上执行查询并返回字符串行集（SHOW/PRAGMA/信息_schema 类）
async fn query_string_rows(entry: &DbSessionEntry, sql: &str) -> Result<Vec<Vec<String>>, String> {
    match &entry.session {
        DbSession::Mysql(pool) => {
            let mut conn = pool
                .get_conn()
                .await
                .map_err(|e| format!("取连接失败: {e}"))?;
            let rows: Vec<mysql_async::Row> = conn
                .query(sql)
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            Ok(rows
                .iter()
                .map(|row| {
                    (0..row.len())
                        .map(|i| {
                            row.get::<Option<String>, usize>(i)
                                .flatten()
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .collect())
        }
        DbSession::Postgres(pool) => {
            let client = pool.get().await.map_err(|e| format!("取连接失败: {e}"))?;
            let rows = client
                .query(sql, &[])
                .await
                .map_err(|e| format!("查询失败: {e}"))?;
            Ok(rows
                .iter()
                .map(|row| {
                    (0..row.len())
                        .map(|i| row.try_get::<usize, String>(i).unwrap_or_default())
                        .collect()
                })
                .collect())
        }
        DbSession::Sqlite(conn) => {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            let mut stmt = guard.prepare(sql).map_err(|e| format!("查询失败: {e}"))?;
            let cols = stmt.column_count();
            let rows = stmt
                .query_map([], |row| {
                    Ok((0..cols)
                        .map(|i| row.get::<_, String>(i).unwrap_or_default())
                        .collect::<Vec<String>>())
                })
                .map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.map_err(|e| e.to_string())?);
            }
            Ok(out)
        }
        DbSession::Redis(_) => Err("Redis 不支持该操作".to_string()),
        DbSession::Agent { .. } => Err("该类型暂不支持在线管理操作".to_string()),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// 建库（字符集/排序规则/用户授权）
// ──────────────────────────────────────────────────────────────────────────

/// 字符集与排序规则选项（新建数据库对话框联动下拉；不支持的类型返回空）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_charset_options(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<DbCharsetOptions, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持字符集选项")?;
    let (Some(charsets_sql), Some(collations_sql)) =
        (dialect.charsets_sql(), dialect.collations_sql())
    else {
        return Ok(DbCharsetOptions {
            charsets: vec![],
            collations_by_charset: Default::default(),
        });
    };
    let charset_rows = query_string_rows(&entry, charsets_sql).await?;
    let collation_rows = query_string_rows(&entry, collations_sql).await?;
    let charsets = charset_rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect();
    let mut collations_by_charset: std::collections::HashMap<String, Vec<String>> =
        Default::default();
    for row in collation_rows {
        if let [charset, collation, ..] = row.as_slice() {
            collations_by_charset
                .entry(charset.clone())
                .or_default()
                .push(collation.clone());
        }
    }
    Ok(DbCharsetOptions {
        charsets,
        collations_by_charset,
    })
}

/// 数据库用户清单（建库授权选择；不支持的类型返回空）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_users(
    state: State<'_, DbState>,
    conn_id: String,
) -> Result<Vec<DbUserInfo>, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持用户清单")?;
    let Some(sql) = dialect.users_sql() else {
        return Ok(vec![]);
    };
    let rows = query_string_rows(&entry, sql).await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            if let [user, host, ..] = r.as_slice() {
                Some(DbUserInfo {
                    user: user.clone(),
                    host: host.clone(),
                })
            } else {
                None
            }
        })
        .collect())
}

/// 新建数据库 + 可选授权（分步执行，逐步返回结果；建库失败即终止）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_create_database(
    state: State<'_, DbState>,
    conn_id: String,
    name: String,
    charset: Option<String>,
    collation: Option<String>,
    grants: Option<Vec<DbGrantInput>>,
) -> Result<Vec<DbStepResult>, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持建库")?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("数据库名不能为空".to_string());
    }
    let sql = dialect
        .create_database_sql(&name, charset.as_deref(), collation.as_deref())
        .ok_or("该类型不支持在线建库")?;

    let mut steps = Vec::new();
    // 第一步：建库（失败即终止，不再授权）
    match exec_admin(&entry, &sql).await {
        Ok(_) => steps.push(DbStepResult {
            label: "创建数据库".to_string(),
            sql: sql.clone(),
            ok: true,
            error: None,
        }),
        Err(e) => {
            steps.push(DbStepResult {
                label: "创建数据库".to_string(),
                sql,
                ok: false,
                // e 之后不再使用，直接移动（规范 §2：不 clone 只用一次的值）
                error: Some(e),
            });
            return Ok(steps);
        }
    }
    // 后续步骤：逐用户授权（单步失败不中断，逐步标错）
    for grant in grants.unwrap_or_default() {
        let label = format!("授权 {}@{}", grant.user, grant.host);
        match dialect.grant_sql(&name, &grant.user, &grant.host, &grant.privilege) {
            Some(grant_sql) => match exec_admin(&entry, &grant_sql).await {
                Ok(_) => steps.push(DbStepResult {
                    label,
                    sql: grant_sql,
                    ok: true,
                    error: None,
                }),
                Err(e) => steps.push(DbStepResult {
                    label,
                    sql: grant_sql,
                    ok: false,
                    error: Some(e),
                }),
            },
            None => steps.push(DbStepResult {
                label,
                sql: String::new(),
                ok: false,
                error: Some(format!("未知权限级别: {}", grant.privilege)),
            }),
        }
    }
    Ok(steps)
}

/// 删除数据库（危险操作，前端弹窗确认后才调用）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_drop_database(
    state: State<'_, DbState>,
    conn_id: String,
    name: String,
) -> Result<String, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持删库")?;
    let sql = dialect
        .drop_database_sql(name.trim())
        .ok_or("该类型不支持删库")?;
    exec_admin(&entry, &sql).await?;
    Ok(sql)
}

// ──────────────────────────────────────────────────────────────────────────
// 表维护（重命名/清空/删除）
// ──────────────────────────────────────────────────────────────────────────

/// 表维护操作统一入口（rename/truncate/drop），返回实际执行的 SQL 供前端提示
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_admin(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
    action: String,
    new_name: Option<String>,
    kind: Option<String>,
) -> Result<String, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持表维护")?;
    let schema = schema.unwrap_or_default();
    let table = table.trim().to_string();
    if table.is_empty() {
        return Err("表名不能为空".to_string());
    }
    let sql = match action.as_str() {
        "rename" => {
            let new = new_name.unwrap_or_default().trim().to_string();
            if new.is_empty() {
                return Err("新表名不能为空".to_string());
            }
            dialect.rename_table_sql(&schema, &table, &new)
        }
        "truncate" => dialect.truncate_table_sql(&schema, &table),
        "drop" => dialect.drop_object_sql(&schema, kind.as_deref().unwrap_or("table"), &table),
        other => return Err(format!("未知表维护操作: {other}")),
    }
    .ok_or("该类型不支持此操作")?;
    exec_admin(&entry, &sql).await?;
    Ok(sql)
}

// ──────────────────────────────────────────────────────────────────────────
// 结构多维信息（DDL / 索引）
// ──────────────────────────────────────────────────────────────────────────

/// 表 DDL（mysql SHOW CREATE TABLE / sqlite sqlite_master；pg 等暂不支持返回提示）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_ddl(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
) -> Result<String, String> {
    let entry = session(&state, &conn_id)?;
    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持查看 DDL")?;
    // mysql 的 schema 段是库名：未传时回退连接默认库
    let schema = schema.unwrap_or_else(|| entry.config.database.clone());
    let sql = dialect
        .table_ddl_sql(&schema, table.trim())
        .ok_or("该类型暂不支持查看 DDL")?;
    let rows = query_string_rows(&entry, &sql).await?;
    // SHOW CREATE TABLE 第二列是 DDL；sqlite_master 第一列即 sql
    let ddl = rows
        .first()
        .map(|r| r.last().cloned().unwrap_or_default())
        .unwrap_or_default();
    if ddl.is_empty() {
        return Err(format!("未取到 {} 的 DDL", table.trim()));
    }
    Ok(ddl)
}

/// 表索引清单（mysql/pg 走方言 SQL；sqlite 用 PRAGMA 两段式拼）
#[tauri::command(rename_all = "camelCase")]
pub async fn dbc_table_indexes(
    state: State<'_, DbState>,
    conn_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<DbIndexInfo>, String> {
    let entry = session(&state, &conn_id)?;
    let table = table.trim().to_string();

    // sqlite：PRAGMA index_list + index_info 两段式（方言 SQL 一次取不回）
    if let DbSession::Sqlite(conn) = &entry.session {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let safe = table.replace(['"', '`'], "");
        let mut stmt = guard
            .prepare(&format!("PRAGMA index_list(\"{safe}\")"))
            .map_err(|e| format!("索引查询失败: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (name, non_unique) = row.map_err(|e| e.to_string())?;
            let safe_idx = name.replace(['"', '`'], "");
            let mut col_stmt = guard
                .prepare(&format!("PRAGMA index_info(\"{safe_idx}\")"))
                .map_err(|e| format!("索引列查询失败: {e}"))?;
            let cols = col_stmt
                .query_map([], |r| r.get::<_, String>(2))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            out.push(DbIndexInfo {
                name,
                columns: cols,
                non_unique: non_unique == 0,
                definition: String::new(),
            });
        }
        return Ok(out);
    }

    let dialect = dialect_for(entry.config.db_type).ok_or("该类型不支持索引查询")?;
    let schema = schema.unwrap_or_else(|| entry.config.database.clone());
    let sql = dialect
        .indexes_sql(&schema, &table)
        .ok_or("该类型暂不支持索引查询")?;
    let rows = query_string_rows(&entry, &sql).await?;
    // 按索引名聚合列（mysql information_schema.STATISTICS 一行一列；pg 一行一索引）
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, DbIndexInfo> = Default::default();
    for row in rows {
        if let [name, column, non_unique, definition, ..] = row.as_slice() {
            let item = map.entry(name.clone()).or_insert_with(|| {
                order.push(name.clone());
                DbIndexInfo {
                    name: name.clone(),
                    columns: vec![],
                    non_unique: non_unique != "0",
                    definition: definition.clone(),
                }
            });
            if !column.is_empty() {
                item.columns.push(column.clone());
            }
        }
    }
    Ok(order.into_iter().filter_map(|n| map.remove(&n)).collect())
}

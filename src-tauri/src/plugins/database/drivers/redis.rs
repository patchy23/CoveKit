//! Redis 键浏览与命令执行辅助（键树 SCAN / 键信息 TYPE+TTL+预览 / 命令运行器）

use crate::plugins::database::models::RedisKeyInfo;
use redis::aio::ConnectionManager;
use redis::Value as RedisValue;

/// SCAN 键列表（游标分页；pattern 为空时全量）
pub(crate) async fn scan_keys(
    mgr: &mut ConnectionManager,
    pattern: &str,
    cursor: u64,
    count: u64,
) -> Result<(u64, Vec<String>), String> {
    let mut cmd = redis::cmd("SCAN");
    cmd.arg(cursor);
    if !pattern.trim().is_empty() {
        cmd.arg("MATCH").arg(pattern.trim());
    }
    cmd.arg("COUNT").arg(count.clamp(10, 1000));
    let value: RedisValue = cmd
        .query_async(mgr)
        .await
        .map_err(|e| format!("SCAN 失败: {e}"))?;
    let (next, keys) = parse_scan(value);
    Ok((next, keys))
}

/// 键信息：TYPE + TTL + 值预览（集合类返回条数与前若干元素）
pub(crate) async fn key_info(
    mgr: &mut ConnectionManager,
    key: &str,
) -> Result<RedisKeyInfo, String> {
    let kind: String = redis::cmd("TYPE")
        .arg(key)
        .query_async(mgr)
        .await
        .map_err(|e| format!("TYPE 失败: {e}"))?;
    let ttl: i64 = redis::cmd("TTL")
        .arg(key)
        .query_async(mgr)
        .await
        .map_err(|e| format!("TTL 失败: {e}"))?;
    let value = preview_value(mgr, key, &kind).await?;
    Ok(RedisKeyInfo {
        key: key.to_string(),
        kind: kind.clone(),
        ttl,
        value,
    })
}

/// 执行一条 Redis 命令（查询页签输入行，如 `GET user:1`）；返回格式化结果
pub(crate) async fn exec_command(
    mgr: &mut ConnectionManager,
    line: &str,
) -> Result<String, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("命令不能为空".to_string());
    }
    let mut parts = split_command(trimmed);
    if parts.is_empty() {
        return Err("命令格式错误".to_string());
    }
    let command = parts.remove(0);
    let mut cmd = redis::cmd(&command);
    for arg in parts {
        cmd.arg(arg);
    }
    let value: RedisValue = cmd
        .query_async(mgr)
        .await
        .map_err(|e| format!("命令执行失败: {e}"))?;
    Ok(render_value(&value))
}

/// 键值预览：按类型取前 10 个元素或直接取值
async fn preview_value(
    mgr: &mut ConnectionManager,
    key: &str,
    kind: &str,
) -> Result<String, String> {
    match kind {
        "string" => {
            let value: Option<String> = redis::cmd("GET")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| format!("GET 失败: {e}"))?;
            Ok(value.unwrap_or_default())
        }
        "list" => {
            let len: i64 = redis::cmd("LLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<String> = redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(9)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("list（共 {len} 项）: [{}]", items.join(", ")))
        }
        "set" => {
            let len: i64 = redis::cmd("SCARD")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<String> = redis::cmd("SMEMBERS")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let shown: Vec<String> = items.into_iter().take(10).collect();
            Ok(format!("set（共 {len} 项）: [{}]", shown.join(", ")))
        }
        "zset" => {
            let len: i64 = redis::cmd("ZCARD")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<String> = redis::cmd("ZRANGE")
                .arg(key)
                .arg(0)
                .arg(9)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("zset（共 {len} 项）: [{}]", items.join(", ")))
        }
        "hash" => {
            let len: i64 = redis::cmd("HLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<String> = redis::cmd("HGETALL")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let pairs: Vec<String> = items
                .chunks(2)
                .take(10)
                .map(|pair| format!("{}: {}", pair[0], pair.get(1).map_or("", |v| v.as_str())))
                .collect();
            Ok(format!("hash（共 {len} 项）: {{ {} }}", pairs.join(", ")))
        }
        "stream" => {
            let len: i64 = redis::cmd("XLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("stream（共 {len} 条消息，详情请用 XRANGE 查看）"))
        }
        other => Ok(format!("{other} 类型（预览暂不支持）")),
    }
}

/// 解析 SCAN 返回：([next_cursor, [keys...]])
fn parse_scan(value: RedisValue) -> (u64, Vec<String>) {
    if let RedisValue::Array(items) = value {
        let mut iter = items.into_iter();
        let next = match iter.next() {
            Some(RedisValue::Int(n)) => n as u64,
            Some(RedisValue::BulkString(b)) => String::from_utf8_lossy(&b).parse().unwrap_or(0),
            _ => 0,
        };
        let keys = match iter.next() {
            Some(RedisValue::Array(list)) => list
                .into_iter()
                .filter_map(|v| match v {
                    RedisValue::BulkString(b) => Some(String::from_utf8_lossy(&b).to_string()),
                    RedisValue::SimpleString(s) => Some(s),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        (next, keys)
    } else {
        (0, Vec::new())
    }
}

/// 命令行拆分：引号包裹的参数原样保留（如 `SET k "hello world"`）
fn split_command(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                ' ' | '\t' => {
                    if !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(c),
            },
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Redis 值渲染为展示文本
pub fn render_value(value: &RedisValue) -> String {
    match value {
        RedisValue::Nil => "(nil)".to_string(),
        RedisValue::Int(n) => n.to_string(),
        RedisValue::BulkString(b) => String::from_utf8_lossy(b).to_string(),
        RedisValue::SimpleString(s) => s.clone(),
        RedisValue::Okay => "OK".to_string(),
        RedisValue::Array(items) => {
            let rendered: Vec<String> = items.iter().map(render_value).collect();
            format!("[{}]", rendered.join(", "))
        }
        _ => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 命令行拆分支持引号() {
        assert_eq!(
            split_command("SET user:1 \"hello world\""),
            vec!["SET", "user:1", "hello world"]
        );
        assert_eq!(split_command("  GET  user:1  "), vec!["GET", "user:1"]);
        assert_eq!(
            split_command("LPUSH list 'a b' c"),
            vec!["LPUSH", "list", "a b", "c"]
        );
        assert_eq!(split_command(""), Vec::<String>::new());
    }

    #[test]
    fn scan结果解析() {
        let value = RedisValue::Array(vec![
            RedisValue::Int(42),
            RedisValue::Array(vec![RedisValue::BulkString(b"k1".to_vec())]),
        ]);
        let (next, keys) = parse_scan(value);
        assert_eq!(next, 42);
        assert_eq!(keys, vec!["k1"]);
    }

    #[test]
    fn 值渲染() {
        assert_eq!(render_value(&RedisValue::Nil), "(nil)");
        assert_eq!(render_value(&RedisValue::Int(7)), "7");
        assert_eq!(render_value(&RedisValue::Okay), "OK");
        assert_eq!(render_value(&RedisValue::BulkString(b"hi".to_vec())), "hi");
    }
}

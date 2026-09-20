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
    parse_scan(value)
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
    let mut parts = split_redis_command(trimmed)?;
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
            let value: Vec<u8> = redis::cmd("GETRANGE")
                .arg(key)
                .arg(0)
                .arg(65535)
                .query_async(mgr)
                .await
                .map_err(|e| format!("GET 失败: {e}"))?;
            let length: u64 = redis::cmd("STRLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let preview = render_bytes(&value);
            Ok(if length > value.len() as u64 {
                format!(
                    "{preview}\n[仅预览前 {} 字节，共 {length} 字节]",
                    value.len()
                )
            } else {
                preview
            })
        }
        "list" => {
            let len: i64 = redis::cmd("LLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<Vec<u8>> = redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(9)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "list（共 {len} 项）: [{}]",
                items
                    .iter()
                    .map(|item| render_bytes(item))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
        "set" => {
            let len: i64 = redis::cmd("SCARD")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let (_, items): (u64, Vec<Vec<u8>>) = redis::cmd("SSCAN")
                .arg(key)
                .arg(0)
                .arg("COUNT")
                .arg(10)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let shown: Vec<String> = items
                .iter()
                .take(10)
                .map(|item| render_bytes(item))
                .collect();
            Ok(format!("set（共 {len} 项）: [{}]", shown.join(", ")))
        }
        "zset" => {
            let len: i64 = redis::cmd("ZCARD")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<Vec<u8>> = redis::cmd("ZRANGE")
                .arg(key)
                .arg(0)
                .arg(9)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "zset（共 {len} 项）: [{}]",
                items
                    .iter()
                    .map(|item| render_bytes(item))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
        "hash" => {
            let len: i64 = redis::cmd("HLEN")
                .arg(key)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let (_, items): (u64, Vec<Vec<u8>>) = redis::cmd("HSCAN")
                .arg(key)
                .arg(0)
                .arg("COUNT")
                .arg(10)
                .query_async(mgr)
                .await
                .map_err(|e| e.to_string())?;
            let pairs: Vec<String> = items
                .chunks(2)
                .take(10)
                .map(|pair| {
                    format!(
                        "{}: {}",
                        render_bytes(&pair[0]),
                        pair.get(1).map(|v| render_bytes(v)).unwrap_or_default()
                    )
                })
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
fn parse_scan(value: RedisValue) -> Result<(u64, Vec<String>), String> {
    let RedisValue::Array(items) = value else {
        return Err("SCAN 返回格式无效".into());
    };
    if items.len() != 2 {
        return Err("SCAN 返回格式无效".into());
    }
    let mut iter = items.into_iter();
    let next = match iter.next() {
        Some(RedisValue::Int(n)) => u64::try_from(n).map_err(|_| "SCAN 游标无效")?,
        Some(RedisValue::BulkString(b)) => std::str::from_utf8(&b)
            .map_err(|_| "SCAN 游标编码无效")?
            .parse()
            .map_err(|_| "SCAN 游标无效")?,
        _ => return Err("SCAN 游标缺失".into()),
    };
    let Some(RedisValue::Array(keys)) = iter.next() else {
        return Err("SCAN 键列表缺失".into());
    };
    let keys = keys
        .into_iter()
        .map(|value| match value {
            RedisValue::BulkString(b) => String::from_utf8(b).map_err(|_| {
                "当前键树不支持非 UTF-8 键名，请使用 Redis 原生字节客户端处理该键".into()
            }),
            RedisValue::SimpleString(s) => Ok(s),
            _ => Err("SCAN 键名格式无效".into()),
        })
        .collect::<Result<_, String>>()?;
    Ok((next, keys))
}

/// 二进制预览保持原始字节，非法 UTF-8 不替换为伪文本。
fn render_bytes(bytes: &[u8]) -> String {
    let preview = &bytes[..bytes.len().min(65536)];
    let value = match std::str::from_utf8(preview) {
        Ok(text) => text.to_string(),
        Err(_) => format!("hex:{}", hex::encode(preview)),
    };
    if bytes.len() > preview.len() {
        format!("{value} [已截断，共 {} 字节]", bytes.len())
    } else {
        value
    }
}

/// 命令参数保留空字符串与反斜线转义；实际执行以结构化参数传给驱动。
pub(crate) fn split_redis_command(line: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut value = String::new();
    let mut quote = None;
    let mut started = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            value.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c => c,
            });
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            started = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                value.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
            started = true;
        } else if ch.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut value));
                started = false;
            }
        } else {
            value.push(ch);
            started = true;
        }
    }
    if escaped || quote.is_some() {
        return Err("Redis 命令存在未闭合引号或转义".into());
    }
    if started {
        args.push(value);
    }
    Ok(args)
}

/// Redis 值渲染为展示文本
pub fn render_value(value: &RedisValue) -> String {
    match value {
        RedisValue::Nil => "(nil)".to_string(),
        RedisValue::Int(n) => n.to_string(),
        RedisValue::BulkString(b) => render_bytes(b),
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
    fn command_split_keeps_quoted_args() {
        assert_eq!(
            split_redis_command("SET user:1 \"hello world\"").unwrap(),
            vec!["SET", "user:1", "hello world"]
        );
        assert_eq!(
            split_redis_command("  GET  user:1  ").unwrap(),
            vec!["GET", "user:1"]
        );
        assert_eq!(
            split_redis_command("LPUSH list 'a b' c").unwrap(),
            vec!["LPUSH", "list", "a b", "c"]
        );
        assert_eq!(split_redis_command("").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn scan结果解析() {
        let value = RedisValue::Array(vec![
            RedisValue::Int(42),
            RedisValue::Array(vec![RedisValue::BulkString(b"k1".to_vec())]),
        ]);
        let (next, keys) = parse_scan(value).unwrap();
        assert_eq!(next, 42);
        assert_eq!(keys, vec!["k1"]);
    }

    #[test]
    fn value_rendering() {
        assert_eq!(render_value(&RedisValue::Nil), "(nil)");
        assert_eq!(render_value(&RedisValue::Int(7)), "7");
        assert_eq!(render_value(&RedisValue::Okay), "OK");
        assert_eq!(render_value(&RedisValue::BulkString(b"hi".to_vec())), "hi");
    }
}

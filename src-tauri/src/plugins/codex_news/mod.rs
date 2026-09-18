//! Codex 消息工具：读取固定公共消息源，持久化消息缓存与阅读状态；不接触账户额度。

use std::time::Duration;

use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::framework::store::PluginDb;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MIGRATIONS: &[&str] =
    &["CREATE TABLE news_state (id INTEGER PRIMARY KEY CHECK (id = 1), value TEXT NOT NULL);"];

/// 两份源文档原样传输；第三方字段校验和归一化由前端本模块负责。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeeds {
    /// 当前动态、事件详情及数据新鲜度。
    status: Value,
    /// 已发布消息的历史索引。
    timeline: Value,
}

async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("获取消息失败：{e}"))?
        .error_for_status()
        .map_err(|e| format!("消息源响应异常：{e}"))?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("读取消息失败：{e}"))?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("消息源响应过大".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("消息源 JSON 无效：{e}"))
}

/// 只请求两个固定 HTTPS 地址；限制重定向、请求时间及体积，无后台常驻任务。
#[tauri::command]
pub async fn codex_news_fetch() -> Result<NewsFeeds, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("CoveKit Codex news")
        .build()
        .map_err(|e| e.to_string())?;
    let (status, timeline) = tokio::try_join!(
        fetch_json(&client, "https://savemetibo.com/status.json"),
        fetch_json(&client, "https://savemetibo.com/timeline.json"),
    )?;
    Ok(NewsFeeds { status, timeline })
}

/// 从当前空间读取缓存；连接只在命令期间持有，维护与退出不遗留句柄。
#[tauri::command]
pub fn codex_news_load(app: AppHandle) -> Result<Option<Value>, String> {
    PluginDb::open(&app, "codex_news", MIGRATIONS)?.with_conn(|conn| {
        let raw: Option<String> = conn
            .query_row("SELECT value FROM news_state WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        raw.map(|value| serde_json::from_str(&value).map_err(|e| format!("消息缓存损坏：{e}")))
            .transpose()
    })
}

/// 保存有界 JSON 快照；数据库路径和维护访问守卫均走框架。
#[tauri::command(rename_all = "camelCase")]
pub fn codex_news_save(app: AppHandle, value: Value) -> Result<(), String> {
    let raw = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    if !value.is_object() || raw.len() > MAX_BYTES {
        return Err("消息缓存格式无效或过大".into());
    }
    PluginDb::open(&app, "codex_news", MIGRATIONS)?.with_conn(|conn| {
        conn.execute("INSERT INTO news_state (id, value) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET value = excluded.value", [raw])
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

crate::covekit_module! {
    owner: "codex_news",
    feature: "codex-news",
    storage: "codex_news",
    commands: {
        codex_news_fetch => "获取 Codex 重置消息源",
        codex_news_load => "读取 Codex 消息缓存与阅读状态",
        codex_news_save => "保存 Codex 消息缓存与阅读状态",
    },
}

/// 无常驻资源；注册命令并沿框架路由装配。
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    builder
}

//! Codex 消息工具：读取固定公共消息源，持久化消息缓存与阅读状态；不接触账户额度。

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::framework::store::PluginDb;

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE news_state (id INTEGER PRIMARY KEY CHECK (id = 1), value TEXT NOT NULL);",
    "CREATE TABLE news_preferences (id INTEGER PRIMARY KEY CHECK (id = 1), value TEXT NOT NULL);",
];
mod storage;

/// AIHOT 完整快照原样传输；第三方字段校验和归一化由前端本模块负责。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeeds {
    /// 重置事件、中文来源帖子与监控新鲜度。
    resets: Value,
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

/// 只请求固定 HTTPS 地址；限制重定向、请求时间及体积，无后台常驻任务。
#[tauri::command]
pub async fn codex_news_fetch() -> Result<NewsFeeds, String> {
    let log_started = std::time::Instant::now();
    let result: Result<NewsFeeds, String> = async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("CoveKit Codex news")
            .build()
            .map_err(|e| e.to_string())?;
        let resets = fetch_json(&client, "https://aihot.news/api/v1/codex-resets").await?;
        Ok(NewsFeeds { resets })
    }
    .await;
    match &result {
        Ok(_value) => log::debug!(
            "操作完成 operation=codex_news_fetch elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=codex_news_fetch elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 从当前空间读取缓存；连接只在命令期间持有，维护与退出不遗留句柄。
#[tauri::command]
pub fn codex_news_load(app: AppHandle) -> Result<Option<Value>, String> {
    let log_started = std::time::Instant::now();
    let result: Result<Option<Value>, String> = (|| {
        PluginDb::open(&app, "codex_news", MIGRATIONS)?.with_conn(storage::load)
    })();
    match &result {
        Ok(_value) => log::debug!(
            "操作完成 operation=codex_news_load elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=codex_news_load elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

/// 保存完整快照或仅阅读状态；正文与状态分开落盘，合计沿用原缓存预算。
#[tauri::command(rename_all = "camelCase")]
pub fn codex_news_save(app: AppHandle, value: Value) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = (|| {
        PluginDb::open(&app, "codex_news", MIGRATIONS)?
            .with_transaction(|conn| storage::save(conn, value))
    })();
    match &result {
        Ok(_value) => log::debug!(
            "操作完成 operation=codex_news_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::error!(
            "操作未完成 operation=codex_news_save elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
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

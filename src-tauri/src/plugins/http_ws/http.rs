//! HTTP/WS 调试插件 · HTTP 请求实现（reqwest）
//! 发送 HTTP 请求（失败返回 ok=false + error，不抛错——便于前端展示）

use std::time::{Duration, Instant};

use crate::plugins::http_ws::models::{HttpRequestPayload, HttpResponseResult};

/// 方法字符串 → reqwest 方法枚举（未知值回退 GET）
fn parse_method(m: &str) -> reqwest::Method {
    match m.to_uppercase().as_str() {
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "PATCH" => reqwest::Method::PATCH,
        "DELETE" => reqwest::Method::DELETE,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    }
}

/// 发送 HTTP 请求（失败返回 ok=false + error，不抛错——便于前端展示）
#[tauri::command]
pub async fn http_request(payload: HttpRequestPayload) -> Result<HttpResponseResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(payload.timeout_ms.unwrap_or(15_000)))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.request(parse_method(&payload.method), &payload.url);
    for (k, v) in &payload.headers {
        if !k.trim().is_empty() {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    if let Some(body) = &payload.body {
        if !body.is_empty() {
            req = req.body(body.clone());
        }
    }

    let start = Instant::now();
    let resp = req.send().await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let status_text = r.status().canonical_reason().unwrap_or("").to_string();
            let headers = r
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<Vec<_>>();
            let body = r.text().await.unwrap_or_default();
            let body_size = body.len();
            Ok(HttpResponseResult {
                ok: status < 400,
                status,
                status_text,
                headers,
                body,
                body_size,
                duration_ms,
                error: None,
            })
        }
        Err(e) => Ok(HttpResponseResult {
            ok: false,
            status: 0,
            status_text: String::new(),
            headers: Vec::new(),
            body: String::new(),
            body_size: 0,
            duration_ms,
            error: Some(e.to_string()),
        }),
    }
}

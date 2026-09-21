//! HTTP 请求与 SSE 共用请求构建，限制响应内存并完整报告读取失败。
use super::models::{HttpRequestPayload, HttpResponseResult};
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// 流式请求只限制建连；普通 HTTP 同时限制整次请求。
pub(super) fn request(
    app: &AppHandle,
    payload: HttpRequestPayload,
    stream: bool,
) -> Result<reqwest::RequestBuilder, String> {
    let url = reqwest::Url::parse(&payload.url).map_err(|_| "请求地址无效".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("请求地址必须使用 HTTP 或 HTTPS".into());
    }
    let timeout = Duration::from_millis(payload.timeout_ms.unwrap_or(15_000).clamp(1_000, 300_000));
    let mut builder = reqwest::Client::builder().connect_timeout(timeout);
    if !stream {
        builder = builder.timeout(timeout);
    }
    let client = builder.build().map_err(|e| e.without_url().to_string())?;
    let method = reqwest::Method::from_bytes(payload.method.as_bytes())
        .map_err(|_| "HTTP 方法无效".to_string())?;
    let mut headers = reqwest::header::HeaderMap::new();
    for (key, value) in payload.headers {
        if key.trim().is_empty() {
            continue;
        }
        let key = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| "请求头名称无效".to_string())?;
        let value = reqwest::header::HeaderValue::from_str(&value)
            .map_err(|_| "请求头值无效".to_string())?;
        headers.append(key, value);
    }
    if let Some(auth) = super::auth::authorization(app, payload.auth.as_ref())? {
        headers.insert(reqwest::header::AUTHORIZATION, auth);
    }
    let mut request = client.request(method, url).headers(headers);
    if let Some(body) = payload.body {
        request = request.body(body);
    }
    Ok(request)
}

/// 保留重复响应头，非 ASCII 内容按可显示文本转换。
pub(super) fn response_headers(response: &reqwest::Response) -> Vec<(String, String)> {
    response
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect()
}

/// 返回完整状态与响应原文；传输中断和过大响应均可感知。
#[tauri::command]
pub async fn http_request(
    app: AppHandle,
    payload: HttpRequestPayload,
) -> Result<HttpResponseResult, String> {
    let log_started = std::time::Instant::now();
    let result: Result<HttpResponseResult, String> = async {
        let request = request(&app, payload, false)?;
        let start = Instant::now();
        let mut response = request
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let status = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let headers = response_headers(&response);
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| e.without_url().to_string())?
        {
            if bytes.len() + chunk.len() > 20 * 1024 * 1024 {
                return Err("响应超过 20 MiB，请缩小请求范围".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(HttpResponseResult {
            ok: status < 400,
            status,
            status_text,
            headers,
            body_size: bytes.len(),
            body: String::from_utf8_lossy(&bytes).into_owned(),
            duration_ms: start.elapsed().as_millis() as u64,
            error: None,
        })
    }
    .await;
    match &result {
        Ok(value) => log::info!(
            "HTTP 请求结束 status={} ok={} bytes={} elapsed_ms={}",
            value.status,
            value.ok,
            value.body_size,
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=http_request elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

//! 协议共用认证：通过框架解析凭证，不将秘密放入错误文本。
use reqwest::header::{HeaderValue, AUTHORIZATION};
use tauri::AppHandle;

use super::models::RequestAuth;
use crate::framework::vault::{resolve, CredentialFields};

/// 只构建请求头，不发送网络请求；同一 HeaderValue 可用于三种协议。
pub(super) fn authorization(
    app: &AppHandle,
    auth: Option<&RequestAuth>,
) -> Result<Option<HeaderValue>, String> {
    let Some(auth) = auth.filter(|value| value.mode != "none") else {
        return Ok(None);
    };
    let (username, secret) = if auth.credential_id.is_empty() {
        (
            auth.username.clone().unwrap_or_default(),
            auth.secret.clone().unwrap_or_default(),
        )
    } else {
        match (
            auth.mode.as_str(),
            resolve(app, &auth.credential_id)?.fields,
        ) {
            ("basic", CredentialFields::Password { username, password }) => (username, password),
            ("bearer", CredentialFields::ApiToken { token }) => (String::new(), token),
            _ => return Err("凭证类型与接口认证方式不匹配".into()),
        }
    };
    let builder = reqwest::Client::builder()
        .build()
        .map_err(|e| e.without_url().to_string())?
        .get("http://localhost/");
    let request = match auth.mode.as_str() {
        "basic" => builder.basic_auth(username, Some(secret)),
        "bearer" if !secret.is_empty() => builder.bearer_auth(secret),
        "bearer" => return Err("Token 不能为空".into()),
        _ => return Err("不支持的认证方式".into()),
    }
    .build()
    .map_err(|_| "认证信息不能用于请求头".to_string())?;
    Ok(request.headers().get(AUTHORIZATION).cloned())
}

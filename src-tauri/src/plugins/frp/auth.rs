//! FRP Token 凭证引用与子进程注入。档案仅保存环境变量模板，Vault 明文不回传、不写文件。

use std::path::{Path, PathBuf};
use tauri::AppHandle;

const PREFIX: &str = "{{ .Envs.COVEKIT_FRP_TOKEN_";
const SUFFIX: &str = " }}";
const ENV_NAME: &str = "COVEKIT_FRP_RUNTIME_TOKEN";
const RUNTIME_TEMPLATE: &str = "{{ .Envs.COVEKIT_FRP_RUNTIME_TOKEN }}";

// 仅用于定位 Token 字面量的源码区间；其余字段交给原文保留，不重建整份配置。
#[derive(serde::Deserialize)]
struct TokenDocument {
    auth: TokenField,
}

#[derive(serde::Deserialize)]
struct TokenField {
    token: toml::Spanned<String>,
}

/// 凭证标识编码为合法 Go 模板字段名；与前端 frpCredential.ts 一致。
pub(super) fn token_reference(id: &str) -> String {
    format!("{PREFIX}{}{SUFFIX}", hex::encode(id))
}

/// 只识别本工具生成的完整模板，普通 Token 和用户自有环境变量不作改写。
pub(super) fn credential_id(token: &str) -> Result<Option<String>, String> {
    let Some(rest) = token.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let encoded = rest
        .strip_suffix(SUFFIX)
        .ok_or("FRP Token 凭证引用格式无效")?;
    let id = String::from_utf8(hex::decode(encoded).map_err(|_| "FRP Token 凭证引用格式无效")?)
        .map_err(|_| "FRP Token 凭证引用格式无效")?;
    if id.is_empty() {
        return Err("FRP Token 凭证引用不能为空".into());
    }
    if token_reference(&id) != token {
        return Err("FRP Token 凭证引用格式无效，请重新选择凭证".into());
    }
    Ok(Some(id))
}

/// 从完整配置中读取引用；解析错误必须可见，不能按没有引用处理。
pub(super) fn reference_in(text: &str) -> Result<Option<String>, String> {
    if !text.contains(PREFIX) {
        return Ok(None);
    }
    let parsed: toml::Value =
        toml::from_str(text).map_err(|_| "FRP 配置无法解析，请在源码中修正 TOML")?;
    credential_id(
        parsed
            .get("auth")
            .and_then(|auth| auth.get("token"))
            .and_then(toml::Value::as_str)
            .unwrap_or(""),
    )
}

/// 一次启动/校验拥有的配置与秘密；日志读取器共享所有权直到最后一条输出处理结束。
pub(super) struct PreparedConfig {
    pub path: PathBuf,
    temporary: bool,
    token: Option<String>,
    escaped: Option<String>,
}

impl PreparedConfig {
    /// 替换 Token 字面量可能改变列号及多行字符串位置，不把运行配置定位冒充原档案定位。
    pub fn has_generated_file(&self) -> bool {
        self.temporary
    }

    /// 仅为当前子进程设置环境，不改应用或系统环境。
    pub fn configure(&self, command: &mut tokio::process::Command) {
        // 即使用户环境中存在同名变量，也不得在未解析凭证时被意外继承。
        command.env_remove(ENV_NAME);
        if let Some(value) = &self.escaped {
            command.env(ENV_NAME, value);
        }
    }

    /// 先按实际秘密精确脱敏，再执行通用日志清理，覆盖短 Token 与无关键字回显。
    pub fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();
        for value in [&self.escaped, &self.token].into_iter().flatten() {
            if !value.is_empty() {
                result = result.replace(value, "***");
                // 日志按行消费，包含换行的 Token 也不能以片段形式泄露。
                for line in value.lines().filter(|line| !line.is_empty()) {
                    result = result.replace(line, "***");
                }
            }
        }
        super::verify::clean_line(&result)
    }
}

impl Drop for PreparedConfig {
    fn drop(&mut self) {
        if self.temporary {
            if let Err(error) = std::fs::remove_file(&self.path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "清理临时配置失败：{error_type}",
                        error_type = std::any::type_name_of_val(&error)
                    );
                }
            }
        }
    }
}

fn render_runtime(text: &str) -> Result<String, String> {
    let parsed: toml::Value = toml::from_str(text).map_err(|_| "FRP 配置无法解析")?;
    let auth = parsed
        .get("auth")
        .and_then(toml::Value::as_table)
        .ok_or("FRP 缺少认证配置")?;
    if auth
        .get("method")
        .and_then(toml::Value::as_str)
        .is_some_and(|method| method != "token")
    {
        return Err("Token 凭证仅适用于 token 认证，请检查认证方式".into());
    }
    if auth.contains_key("tokenSource") {
        return Err("Token 凭证与 tokenSource 不能同时使用，请在源码中移除其中一项".into());
    }
    replace_token_literal(text, RUNTIME_TEMPLATE)
}

/// 数据导入时只改 Token 字段中的引用，保留原文其他字段与注释。
pub(super) fn remap_reference(text: &str, id: &str) -> Result<String, String> {
    replace_token_literal(text, &token_reference(id))
}

fn replace_token_literal(text: &str, token: &str) -> Result<String, String> {
    let document: TokenDocument = toml::from_str(text).map_err(|_| "定位 FRP Token 失败")?;
    let span = document.auth.token.span();
    let before = text.get(..span.start).ok_or("FRP Token 源码位置无效")?;
    let after = text.get(span.end..).ok_or("FRP Token 源码位置无效")?;
    // 只换该字段为基础字符串，确保环境注入采用正确转义；保留注释里的 Go 模板指令。
    Ok(format!("{before}\"{token}\"{after}"))
}

/// 在阻塞任务中读取 Vault 和准备配置；临时文件只含模板，Token 通过子进程环境注入。
pub(super) async fn prepare(app: &AppHandle, path: &Path) -> Result<PreparedConfig, String> {
    let app = app.clone();
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("读取 FRP 配置失败：{e}"))?;
        let Some(id) = reference_in(&text)? else {
            return Ok(PreparedConfig {
                path,
                temporary: false,
                token: None,
                escaped: None,
            });
        };
        let rendered = render_runtime(&text)?;
        let credential = crate::framework::vault::resolve(&app, &id)?;
        let crate::framework::vault::CredentialFields::ApiToken { token } = credential.fields
        else {
            return Err("FRP Token 需要 API Token 类型凭证，请重新选择".into());
        };
        if token.is_empty() {
            return Err("FRP Token 凭证为空，请在凭证库中补充".into());
        }
        // JSON 基础字符串转义是 TOML 基础字符串支持的子集；去掉外围双引号后注入。
        let quoted = serde_json::to_string(&token).map_err(|_| "FRP Token 转义失败")?;
        let escaped = quoted[1..quoted.len() - 1].to_string();
        let temporary = crate::framework::paths::cache_dir(&app, "frp")?
            .join(format!("run-{}.toml", uuid::Uuid::new_v4()));
        crate::framework::secure_store::replace_file(&temporary, rendered.as_bytes())?;
        Ok(PreparedConfig {
            path: temporary,
            temporary: true,
            token: Some(token),
            escaped: Some(escaped),
        })
    })
    .await
    .map_err(|e| format!("准备 FRP 凭证失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_tokens_are_redacted_and_temporary_files_are_removed() {
        let path = std::env::temp_dir().join(format!("covekit-frp-auth-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, "template-only").unwrap();
        let config = PreparedConfig {
            path: path.clone(),
            temporary: true,
            token: Some("xy".into()),
            escaped: Some("xy".into()),
        };
        assert_eq!(config.redact("unexpected value xy"), "unexpected value ***");
        drop(config);
        assert!(!path.exists());
    }

    #[test]
    fn references_round_trip_and_reject_malformed_ids() {
        let reference = token_reference("凭证-id");
        assert_eq!(
            reference,
            "{{ .Envs.COVEKIT_FRP_TOKEN_e587ade8af812d6964 }}"
        );
        assert_eq!(
            credential_id(&reference).unwrap().as_deref(),
            Some("凭证-id")
        );
        assert_eq!(credential_id("manual-token").unwrap(), None);
        assert!(credential_id(&format!("{PREFIX}zz{SUFFIX}")).is_err());
    }

    #[test]
    fn runtime_config_preserves_fields_and_escapes_arbitrary_tokens() {
        let original = format!(
            "auth.token = '{}'\ntransport.tls.certFile = 'cert.pem'\n",
            token_reference("id")
        );
        let rendered = render_runtime(&original).unwrap();
        assert!(rendered.contains(&format!("\"{RUNTIME_TEMPLATE}\"")));
        let with_directive = format!("# {{{{ if .Envs.ENABLED }}}}\n{original}# {{{{ end }}}}\n");
        assert_eq!(
            render_runtime(&with_directive).unwrap(),
            with_directive.replace(
                &format!("'{}'", token_reference("id")),
                &format!("\"{RUNTIME_TEMPLATE}\"")
            )
        );
        for token in ["a", "quote\"slash\\line\n", "汉字\t'{{ not_a_template }}"] {
            let quoted = serde_json::to_string(token).unwrap();
            let injected = rendered.replace(RUNTIME_TEMPLATE, &quoted[1..quoted.len() - 1]);
            let parsed: toml::Value = toml::from_str(&injected).unwrap();
            assert_eq!(parsed["auth"]["token"].as_str(), Some(token));
            assert_eq!(
                parsed["transport"]["tls"]["certFile"].as_str(),
                Some("cert.pem")
            );
        }
        assert!(render_runtime("auth.method = 'oidc'\nauth.token = 'x'").is_err());
        assert!(render_runtime("auth.token = 'x'\nauth.tokenSource.type = 'file'").is_err());
    }
}

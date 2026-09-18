//! 框架 · 应用更新可用性（可靠性 T08-4）
//!
//! 为什么需要单独一个判定：仓库里的 `tauri.conf.json` 带着**占位公钥**，
//! 只有正式发布流水线才会用真实公钥覆盖它（`pnpm release:config` + `tauri.release.conf.json`）。
//! 如果不做这个区分，开发构建与普通安装包会出现「点检查更新 → 报错」，甚至把占位配置当成可用通道。
//!
//! 判定口径：公钥缺失、仍是占位值、或没有下载地址，都算**不可用**，并把原因交给界面展示。

use serde::Serialize;
use tauri::AppHandle;

/// 仓库内置的占位公钥：出现在运行时配置里即表示没有真实更新通道
const PLACEHOLDER_PUBKEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IHBsYWNlaG9sZGVyCg==";

/// 更新可用性
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailability {
    /// 是否真的可以检查/安装更新
    pub available: bool,
    /// 不可用原因（可直接展示；可用时为空串）
    pub reason: String,
    /// 更新通道（下载地址），用于诊断展示
    pub channel: String,
}

impl UpdateAvailability {
    /// 不可用
    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: reason.into(),
            channel: String::new(),
        }
    }

    /// 可用
    fn available(channel: String) -> Self {
        Self {
            available: true,
            reason: String::new(),
            channel,
        }
    }
}

/// 依据公钥与下载地址判定更新是否可用（纯函数，便于单测）
pub fn classify(pubkey: Option<&str>, endpoints: &[String]) -> UpdateAvailability {
    let Some(pubkey) = pubkey.map(str::trim).filter(|value| !value.is_empty()) else {
        return UpdateAvailability::unavailable("当前构建未配置更新公钥，更新检查不可用");
    };
    if pubkey == PLACEHOLDER_PUBKEY {
        return UpdateAvailability::unavailable(
            "当前构建使用占位更新公钥（开发或普通安装包），更新检查不可用",
        );
    }
    let endpoints: Vec<String> = endpoints
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    if endpoints.is_empty() {
        return UpdateAvailability::unavailable("当前构建未配置更新下载地址，更新检查不可用");
    }
    UpdateAvailability::available(endpoints.join(" "))
}

/// 读取运行时配置判定更新可用性
#[tauri::command]
pub fn update_availability(app: AppHandle) -> UpdateAvailability {
    let config = app.config();
    let Some(updater) = config.plugins.0.get("updater") else {
        return UpdateAvailability::unavailable("当前构建未启用更新插件，更新检查不可用");
    };
    let pubkey = updater.get("pubkey").and_then(|value| value.as_str());
    let endpoints: Vec<String> = updater
        .get("endpoints")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    classify(pubkey, &endpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 占位公钥必须判为不可用，哪怕下载地址看着是真实的
    #[test]
    fn placeholder_pubkey_is_unavailable() {
        let status = classify(
            Some(PLACEHOLDER_PUBKEY),
            &["https://github.com/patchy23/CoveKit/releases/latest/download/latest.json".into()],
        );
        assert!(!status.available);
        assert!(status.reason.contains("占位更新公钥"), "{}", status.reason);
        assert!(status.channel.is_empty());
    }

    /// 缺公钥、缺地址同样不可用
    #[test]
    fn missing_key_or_endpoint_is_unavailable() {
        assert!(!classify(None, &["https://example.com/latest.json".into()]).available);
        assert!(!classify(Some("   "), &[]).available);
        let no_endpoint = classify(Some("real-key-value"), &[]);
        assert!(!no_endpoint.available);
        assert!(
            no_endpoint.reason.contains("下载地址"),
            "{}",
            no_endpoint.reason
        );
    }

    /// 真实公钥 + 地址才可用，并带上通道信息
    #[test]
    fn real_key_with_endpoint_is_available() {
        let status = classify(
            Some("real-key-value"),
            &["https://updates.example.com/latest.json".into()],
        );
        assert!(status.available);
        assert_eq!(status.channel, "https://updates.example.com/latest.json");
        assert!(status.reason.is_empty());
    }
}

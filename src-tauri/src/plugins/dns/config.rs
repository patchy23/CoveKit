//! DNS 平台配置持久化与 Vault 认证解析。

use crate::framework::store::PluginDb;
use crate::framework::vault;
use crate::framework::vault::Credential;
use crate::framework::vault::CredentialFields;
use crate::plugins::dns::cloudflare;
use crate::plugins::dns::dnspod;
use crate::plugins::dns::models;
use models::DnsConfig;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::State;

/// DNS 插件数据库（惰性打开：首次命令时 open + 迁移）
pub struct DnsState(pub Mutex<Option<PluginDb>>);

/// 数据表迁移（只追加；v1 = 云平台密钥配置表，v2 = 公共 Vault 引用）
pub(super) const MIGRATIONS: &[&str] = &[
    // 云平台密钥（platform 主键；新增平台只追加配置行）
    "CREATE TABLE IF NOT EXISTS dns_config (
        platform TEXT PRIMARY KEY,
        id TEXT NOT NULL DEFAULT '',
        key TEXT NOT NULL DEFAULT ''
    );",
    "ALTER TABLE dns_config ADD COLUMN credential_ref TEXT;",
    "ALTER TABLE dns_config ADD COLUMN credential_pending INTEGER NOT NULL DEFAULT 0;",
];

/// 获取数据库连接（首次自动打开 + 迁移；锁内同步使用，不跨 await）
fn db<'a>(
    app: &'a AppHandle,
    state: &'a State<'_, DnsState>,
) -> Result<std::sync::MutexGuard<'a, Option<PluginDb>>, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(PluginDb::open(app, "dns", MIGRATIONS)?);
    }
    Ok(guard)
}

/// 读取三平台密钥配置（供云解析命令使用；先取出再 await，避免持锁跨 await）
fn load_config(app: &AppHandle, state: &State<'_, DnsState>) -> Result<DnsConfig, String> {
    let guard = db(app, state)?;
    let conn = guard.as_ref().ok_or("本地库未初始化")?;
    conn.with_conn(|c| {
        let mut cfg = DnsConfig::default();
        // 逐行读取密钥表，按 platform 归位
        let mut stmt = c
            .prepare("SELECT platform, id, key, credential_ref FROM dns_config")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (platform, id, key, credential_ref) = row.map_err(|e| e.to_string())?;
            match platform.as_str() {
                models::PLATFORM_ALIYUN => {
                    cfg.aliyun = models::ProviderConfig {
                        id,
                        key,
                        credential_ref,
                    };
                }
                models::PLATFORM_DNSPOD => {
                    cfg.dnspod = models::ProviderConfig {
                        id,
                        key,
                        credential_ref,
                    };
                }
                models::PLATFORM_CLOUDFLARE => {
                    cfg.cloudflare = models::CloudflareConfig {
                        token: key,
                        credential_ref,
                    };
                }
                _ => {}
            }
        }
        Ok(cfg)
    })
}

/// 将公共 Vault 的 API Token 映射为 Cloudflare 配置。
pub(super) fn apply_vault_api_token(
    mut config: models::CloudflareConfig,
    credential: Credential,
) -> Result<models::CloudflareConfig, String> {
    let CredentialFields::ApiToken { token } = credential.fields else {
        return Err("Cloudflare 所选 Vault 凭证不是“API Token”类型".into());
    };
    config.token = token;
    Ok(config)
}

/// 将公共 Vault 的 AccessKey 对映射为云平台配置。
pub(super) fn apply_vault_credential(
    mut config: models::ProviderConfig,
    credential: Credential,
    platform_label: &str,
) -> Result<models::ProviderConfig, String> {
    let CredentialFields::AccessKeyPair {
        access_key_id,
        access_key_secret,
    } = credential.fields
    else {
        return Err(format!(
            "{platform_label} 所选 Vault 凭证不是“AccessKey 对”类型"
        ));
    };
    config.id = access_key_id;
    config.key = access_key_secret;
    Ok(config)
}

/// 解析单个平台的有效配置：有 credentialRef 时优先使用 Vault，否则保留手工输入。
fn resolve_provider_config(
    app: &AppHandle,
    config: models::ProviderConfig,
    platform_label: &str,
) -> Result<models::ProviderConfig, String> {
    let Some(credential_ref) = config
        .credential_ref
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    else {
        return Ok(config);
    };
    let credential = vault::resolve(app, credential_ref)
        .map_err(|e| format!("{platform_label} Vault 凭据读取失败: {e}"))?;
    apply_vault_credential(config, credential, platform_label)
}

/// 解析 Cloudflare 有效配置：Vault API Token 优先，未选择时保留手工 Token。
fn resolve_cloudflare_config(
    app: &AppHandle,
    config: models::CloudflareConfig,
) -> Result<models::CloudflareConfig, String> {
    let Some(credential_ref) = config
        .credential_ref
        .as_deref()
        .filter(|id| !id.trim().is_empty())
    else {
        return Ok(config);
    };
    let credential = vault::resolve(app, credential_ref)
        .map_err(|e| format!("Cloudflare Vault 凭据读取失败: {e}"))?;
    apply_vault_api_token(config, credential)
}

/// 云 API 使用的有效配置；Vault 明文只在 Rust 内存在，不经配置 IPC 返回。
pub(super) fn load_effective_config(
    app: &AppHandle,
    state: &State<'_, DnsState>,
) -> Result<DnsConfig, String> {
    let config = load_config(app, state)?;
    Ok(DnsConfig {
        aliyun: resolve_provider_config(app, config.aliyun, "阿里云")?,
        dnspod: resolve_provider_config(app, config.dnspod, "腾讯云 DNSPod")?,
        cloudflare: resolve_cloudflare_config(app, config.cloudflare)?,
    })
}

/* ── 配置命令 ── */

/// 读取云平台密钥配置（前端设置页回显）
#[tauri::command]
pub fn dns_config_get(app: AppHandle, state: State<'_, DnsState>) -> Result<DnsConfig, String> {
    load_config(&app, &state)
}

/// 保存云平台密钥配置（upsert 三平台；保存后立即生效）
#[tauri::command]
pub fn dns_config_set(
    app: AppHandle,
    state: State<'_, DnsState>,
    config: DnsConfig,
) -> Result<(), String> {
    let log_started = std::time::Instant::now();
    let result: Result<(), String> = (|| {
        let guard = db(&app, &state)?;
        let conn = guard.as_ref().ok_or("本地库未初始化")?;
        conn.with_conn(|c| {
        for (platform, provider) in [
            (models::PLATFORM_ALIYUN, config.aliyun),
            (models::PLATFORM_DNSPOD, config.dnspod),
        ] {
            c.execute(
                "INSERT INTO dns_config (platform, id, key, credential_ref) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(platform) DO UPDATE SET id = ?2, key = ?3, credential_ref = ?4, credential_pending=0",
                rusqlite::params![platform, provider.id, provider.key, provider.credential_ref],
            )
            .map_err(|e| e.to_string())?;
        }
        c.execute(
            "INSERT INTO dns_config (platform, id, key, credential_ref) VALUES (?1, '', ?2, ?3)
             ON CONFLICT(platform) DO UPDATE SET id = '', key = ?2, credential_ref = ?3, credential_pending=0",
            rusqlite::params![
                models::PLATFORM_CLOUDFLARE,
                config.cloudflare.token,
                config.cloudflare.credential_ref
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    })();
    match &result {
        Ok(_value) => log::info!(
            "操作完成 operation=dns_config_set elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
        Err(_) => log::warn!(
            "操作未完成 operation=dns_config_set elapsed_ms={}",
            log_started.elapsed().as_millis()
        ),
    }
    result
}

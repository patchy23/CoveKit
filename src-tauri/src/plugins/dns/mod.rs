//! DNS 插件 · 门面
//! 能力一：DNS 查询（query.rs，hickory-resolver，支持指定服务器/多服务器对比）
//! 能力二：云解析管理（alidns.rs 阿里云 / dnspod.rs 腾讯云 DNSPod API 3.0）
//! 配置：dns.db（PluginDb 统一骨架）存两平台密钥（M3 stronghold 加密升级，当前明文）

mod alidns;
mod dnspod;
mod models;
mod query;

use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::framework::store::PluginDb;
use models::{DnsConfig, DomainList, RecordList, ServerQueryResult};

/// DNS 插件数据库（惰性打开：首次命令时 open + 迁移）
pub struct DnsState(pub Mutex<Option<PluginDb>>);

/// 数据表迁移（只追加；v1 = 云平台密钥配置表）
const MIGRATIONS: &[&str] = &[
    // 云平台密钥（platform 主键，两行：aliyun / dnspod）
    "CREATE TABLE IF NOT EXISTS dns_config (
        platform TEXT PRIMARY KEY,
        id TEXT NOT NULL DEFAULT '',
        key TEXT NOT NULL DEFAULT ''
    );",
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

/// 读取两平台密钥配置（供云解析命令使用；先取出再 await，避免持锁跨 await）
fn load_config(app: &AppHandle, state: &State<'_, DnsState>) -> Result<DnsConfig, String> {
    let guard = db(app, state)?;
    let conn = guard.as_ref().unwrap();
    conn.with_conn(|c| {
        let mut cfg = DnsConfig::default();
        // 逐行读取密钥表，按 platform 归位
        let mut stmt = c
            .prepare("SELECT platform, id, key FROM dns_config")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (platform, id, key) = row.map_err(|e| e.to_string())?;
            match platform.as_str() {
                models::PLATFORM_ALIYUN => {
                    cfg.aliyun = models::ProviderConfig { id, key };
                }
                models::PLATFORM_DNSPOD => {
                    cfg.dnspod = models::ProviderConfig { id, key };
                }
                _ => {}
            }
        }
        Ok(cfg)
    })
}

/* ── 配置命令 ── */

/// 读取云平台密钥配置（前端设置页回显）
#[tauri::command]
pub fn dns_config_get(app: AppHandle, state: State<'_, DnsState>) -> Result<DnsConfig, String> {
    load_config(&app, &state)
}

/// 保存云平台密钥配置（upsert 两平台；保存后立即生效）
#[tauri::command]
pub fn dns_config_set(
    app: AppHandle,
    state: State<'_, DnsState>,
    config: DnsConfig,
) -> Result<(), String> {
    let guard = db(&app, &state)?;
    let conn = guard.as_ref().unwrap();
    conn.with_conn(|c| {
        c.execute(
            "INSERT INTO dns_config (platform, id, key) VALUES ('aliyun', ?1, ?2)
             ON CONFLICT(platform) DO UPDATE SET id = ?1, key = ?2",
            rusqlite::params![config.aliyun.id, config.aliyun.key],
        )
        .map_err(|e| e.to_string())?;
        c.execute(
            "INSERT INTO dns_config (platform, id, key) VALUES ('dnspod', ?1, ?2)
             ON CONFLICT(platform) DO UPDATE SET id = ?1, key = ?2",
            rusqlite::params![config.dnspod.id, config.dnspod.key],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/* ── DNS 查询命令 ── */

/// DNS 查询：对每台服务器依次查询（单台失败不影响其它，包装为 ok=false 结果）
#[tauri::command]
pub async fn dns_query(
    domain: String,
    rtype: String,
    servers: Vec<String>,
) -> Result<Vec<ServerQueryResult>, String> {
    let rtype = query::parse_record_type(&rtype)?;
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err("请输入要查询的域名".into());
    }
    if servers.is_empty() {
        return Err("请至少选择一台 DNS 服务器".into());
    }
    // 逐台串行查询：超时由 hickory 内部兜底（默认 ~5s），单台失败包装为结果
    let mut results = Vec::with_capacity(servers.len());
    for server in &servers {
        let result = query::query_server(&domain, rtype, server)
            .await
            .unwrap_or_else(|e| ServerQueryResult {
                server: server.clone(),
                ok: false,
                error: Some(e),
                elapsed_ms: 0,
                records: vec![],
            });
        results.push(result);
    }
    Ok(results)
}

/* ── 云解析命令 ── */

/// 云解析域名列表（platform: aliyun / dnspod）
#[tauri::command]
pub async fn dns_domains(
    app: AppHandle,
    state: State<'_, DnsState>,
    platform: String,
) -> Result<DomainList, String> {
    let cfg = load_config(&app, &state)?;
    match platform.as_str() {
        models::PLATFORM_ALIYUN => alidns::AliyunDns::new(&cfg.aliyun)?.get_domains().await,
        models::PLATFORM_DNSPOD => dnspod::TencentDns::new(&cfg.dnspod)?.get_domains().await,
        _ => Err(format!("不支持的平台: {platform}")),
    }
}

/// 云解析记录列表（分页；keyword 非空时服务端按主机记录/记录值模糊搜索）
#[tauri::command]
pub async fn dns_records(
    app: AppHandle,
    state: State<'_, DnsState>,
    platform: String,
    domain: String,
    page: u32,
    size: u32,
    keyword: String,
) -> Result<RecordList, String> {
    let cfg = load_config(&app, &state)?;
    match platform.as_str() {
        models::PLATFORM_ALIYUN => {
            alidns::AliyunDns::new(&cfg.aliyun)?
                .get_records(&domain, page.max(1), size.clamp(1, 200), &keyword)
                .await
        }
        models::PLATFORM_DNSPOD => {
            dnspod::TencentDns::new(&cfg.dnspod)?
                .get_records(&domain, page.max(1), size.clamp(1, 200), &keyword)
                .await
        }
        _ => Err(format!("不支持的平台: {platform}")),
    }
}

/// 云解析添加记录（payload 打包，规避 clippy too_many_arguments）
#[tauri::command]
pub async fn dns_add_record(
    app: AppHandle,
    state: State<'_, DnsState>,
    payload: models::AddRecordPayload,
) -> Result<(), String> {
    if payload.rr.trim().is_empty() || payload.value.trim().is_empty() {
        return Err("主机记录与记录值不能为空".into());
    }
    let cfg = load_config(&app, &state)?;
    match payload.platform.as_str() {
        models::PLATFORM_ALIYUN => {
            alidns::AliyunDns::new(&cfg.aliyun)?
                .add_record(
                    &payload.domain,
                    payload.rr.trim(),
                    &payload.rtype,
                    payload.value.trim(),
                    payload.ttl,
                )
                .await
        }
        models::PLATFORM_DNSPOD => {
            dnspod::TencentDns::new(&cfg.dnspod)?
                .add_record(
                    &payload.domain,
                    payload.rr.trim(),
                    &payload.rtype,
                    payload.value.trim(),
                    payload.ttl,
                )
                .await
        }
        _ => Err(format!("不支持的平台: {}", payload.platform)),
    }
}

/// 云解析更新记录（payload 打包，规避 clippy too_many_arguments）
#[tauri::command]
pub async fn dns_update_record(
    app: AppHandle,
    state: State<'_, DnsState>,
    payload: models::UpdateRecordPayload,
) -> Result<(), String> {
    let cfg = load_config(&app, &state)?;
    match payload.platform.as_str() {
        models::PLATFORM_ALIYUN => {
            alidns::AliyunDns::new(&cfg.aliyun)?
                .update_record(
                    &payload.domain,
                    &payload.record_id,
                    payload.rr.trim(),
                    &payload.rtype,
                    payload.value.trim(),
                    payload.ttl,
                )
                .await
        }
        models::PLATFORM_DNSPOD => {
            dnspod::TencentDns::new(&cfg.dnspod)?
                .update_record(
                    &payload.domain,
                    &payload.record_id,
                    payload.rr.trim(),
                    &payload.rtype,
                    payload.value.trim(),
                    payload.ttl,
                )
                .await
        }
        _ => Err(format!("不支持的平台: {}", payload.platform)),
    }
}

/// 云解析删除记录
#[tauri::command]
pub async fn dns_delete_record(
    app: AppHandle,
    state: State<'_, DnsState>,
    platform: String,
    domain: String,
    record_id: String,
) -> Result<(), String> {
    let cfg = load_config(&app, &state)?;
    match platform.as_str() {
        models::PLATFORM_ALIYUN => {
            alidns::AliyunDns::new(&cfg.aliyun)?
                .delete_record(&record_id)
                .await
        }
        models::PLATFORM_DNSPOD => {
            dnspod::TencentDns::new(&cfg.dnspod)?
                .delete_record(&domain, &record_id)
                .await
        }
        _ => Err(format!("不支持的平台: {platform}")),
    }
}

/// 分派 DNS 插件命令。
pub(crate) fn invoke_handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
        dns_query,
        dns_domains,
        dns_records,
        dns_add_record,
        dns_update_record,
        dns_delete_record,
        dns_config_get,
        dns_config_set,
    ];
    handler(invoke)
}

/// 插件注册：命令入库 + State
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    crate::framework::ipc_registry::register(&[
        ("dns_query", "DNS 查询（指定服务器/多服务器对比）"),
        ("dns_domains", "云解析域名列表（aliyun/dnspod）"),
        ("dns_records", "云解析记录列表（分页）"),
        ("dns_add_record", "云解析添加记录"),
        ("dns_update_record", "云解析更新记录"),
        ("dns_delete_record", "云解析删除记录"),
        ("dns_config_get", "读取云平台密钥配置"),
        ("dns_config_set", "保存云平台密钥配置"),
    ])
    .expect("IPC 命令重复注册");
    builder.manage(DnsState(Mutex::new(None)))
}

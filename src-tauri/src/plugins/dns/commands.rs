//! DNS 查询与云解析记录操作命令。

use super::config::load_effective_config;
use super::config::DnsState;
use crate::plugins::dns::alidns;
use crate::plugins::dns::cloudflare;
use crate::plugins::dns::dnspod;
use crate::plugins::dns::models;
use crate::plugins::dns::query;
use models::DomainList;
use models::RecordList;
use models::ServerQueryResult;
use tauri::AppHandle;
use tauri::State;

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

/// 云解析域名列表（platform: aliyun / dnspod / cloudflare）
#[tauri::command]
pub async fn dns_domains(
    app: AppHandle,
    state: State<'_, DnsState>,
    platform: String,
) -> Result<DomainList, String> {
    let cfg = load_effective_config(&app, &state)?;
    match platform.as_str() {
        models::PLATFORM_ALIYUN => alidns::AliyunDns::new(&cfg.aliyun)?.get_domains().await,
        models::PLATFORM_DNSPOD => dnspod::TencentDns::new(&cfg.dnspod)?.get_domains().await,
        models::PLATFORM_CLOUDFLARE => {
            cloudflare::CloudflareDns::new(&cfg.cloudflare)?
                .get_domains()
                .await
        }
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
    let cfg = load_effective_config(&app, &state)?;
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
        models::PLATFORM_CLOUDFLARE => {
            cloudflare::CloudflareDns::new(&cfg.cloudflare)?
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
    let cfg = load_effective_config(&app, &state)?;
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
        models::PLATFORM_CLOUDFLARE => {
            cloudflare::CloudflareDns::new(&cfg.cloudflare)?
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
    let cfg = load_effective_config(&app, &state)?;
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
        models::PLATFORM_CLOUDFLARE => {
            cloudflare::CloudflareDns::new(&cfg.cloudflare)?
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
    let cfg = load_effective_config(&app, &state)?;
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
        models::PLATFORM_CLOUDFLARE => {
            cloudflare::CloudflareDns::new(&cfg.cloudflare)?
                .delete_record(&domain, &record_id)
                .await
        }
        _ => Err(format!("不支持的平台: {platform}")),
    }
}

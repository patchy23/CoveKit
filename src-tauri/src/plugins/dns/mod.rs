//! DNS 命令装配与插件注册入口。

mod alidns;
mod aliyun_rpc;
mod cloudflare;
mod credential_refs;
mod dnspod;
mod models;
mod query;
mod requests;
mod transfer;

pub(crate) mod commands;
pub(crate) mod config;
#[cfg(test)]
mod tests;
use self::config::DnsState;
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

/// 页面按请求精确取消；应用退出、空间维护再统一释放全部读请求。
fn on_dispose(
    app: Option<&tauri::AppHandle>,
    _reason: crate::framework::lifecycle::CloseReason,
) -> Vec<String> {
    let Some(app) = app else {
        return Vec::new();
    };
    app.state::<requests::DnsRequests>()
        .cancel_all()
        .err()
        .into_iter()
        .collect()
}

/// 仅复用无凭证的传输连接池；授权头和签名仍由每次调用的有效配置生成。
fn http_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new).clone()
}

/// 插件注册：命令入库 + State
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    transfer::register();
    crate::framework::lifecycle::register(
        crate::framework::lifecycle::ModuleLifecycle::for_tool(IPC_OWNER, "dns")
            .with_dispose(on_dispose)
            .with_storage_reset(release_storage),
    );
    // 凭证引用自报：框架删除凭证前据此判断还有哪些平台配置在用
    credential_refs::register_provider();
    builder
        .manage(DnsState(Mutex::new(None)))
        .manage(requests::DnsRequests::default())
}

crate::covekit_module! {
    owner: "dns",
    feature: "dns",
    commands: {
        commands::dns_read_prepare => "登记可取消的 DNS 只读请求",
        commands::dns_read_cancel => "取消 DNS 只读请求",
        commands::dns_query => "DNS 查询（指定服务器/多服务器对比）",
        commands::dns_domains => "云解析域名列表（aliyun/dnspod/cloudflare）",
        commands::dns_records => "云解析记录列表（分页）",
        commands::dns_add_record => "云解析添加记录",
        commands::dns_update_record => "云解析更新记录",
        commands::dns_delete_record => "云解析删除记录",
        config::dns_config_get => "读取云平台密钥配置",
        config::dns_config_set => "保存云平台密钥配置",
    },
}

/// 维护入口冻结并排空请求后释放配置库句柄，后续访问按当前数据根惰性打开。
pub(crate) fn release_storage(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<DnsState>();
    let mut database = state
        .0
        .try_lock()
        .map_err(|e| format!("DNS 配置库仍在使用: {e}"))?;
    *database = None;
    Ok(())
}

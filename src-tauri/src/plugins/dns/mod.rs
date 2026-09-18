//! DNS 命令装配与插件注册入口。

mod alidns;
mod cloudflare;
mod credential_refs;
mod dnspod;
mod models;
mod query;
mod transfer;

pub(crate) mod commands;
pub(crate) mod config;
#[cfg(test)]
mod tests;
use self::config::DnsState;
use std::sync::Mutex;

/// 插件注册：命令入库 + State
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    register_ipc_or_fail();
    transfer::register();
    // 关闭清理：本插件只有 `DnsState` 里的 PluginDb 句柄，没有会话或子进程需要回收，
    // 故不登记关闭钩子（AR06 方案 §5；新增常驻资源时必须回来补登记）
    // 凭证引用自报：框架删除凭证前据此判断还有哪些平台配置在用
    credential_refs::register_provider();
    builder.manage(DnsState(Mutex::new(None)))
}

crate::covekit_module! {
    owner: "dns",
    feature: "dns",
    commands: {
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

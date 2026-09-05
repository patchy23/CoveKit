//! IPC 接口入库 · 命令注册表（规则见 docs/03-plugin-development.md §2）
//! - 插件 register() 时登记命令（归属者 + 名称 + 中文说明）
//! - 启动时校验全局唯一：重复注册直接 panic（开发期暴露冲突）
//! - 插件路由按注册表精确匹配命令名（owner 字段），不再依赖人工维护的前缀清单——
//!   新增插件忘加前缀导致命令静默落到框架 handler 的事故路径由此消除
//! - 框架命令 framework_commands 返回全量清单（名称 + 说明），供调试/文档生成

use std::sync::Mutex;

/// 命令条目（入库最小单位）
#[derive(Clone)]
pub struct IpcEntry {
    /// 命令名（Tauri invoke 名，全局唯一）
    pub name: &'static str,
    /// 中文用途说明（供 framework_commands 查询与文档生成）
    pub doc: &'static str,
    /// 归属者：插件 id（"ssh"/"dns"…）或 "framework"（设置/Vault 等框架命令）
    pub owner: &'static str,
}

static REGISTRY: Mutex<Option<Vec<IpcEntry>>> = Mutex::new(None);

/// 插件 register() 调用：登记本插件命令清单
/// owner 为插件 id（与 plugins/<id>/ 目录同名）；框架命令用 "framework"。
/// 返回 Err = 命令名冲突（重复注册），由调用方 panic 暴露
pub fn register(
    owner: &'static str,
    entries: &[(&'static str, &'static str)],
) -> Result<(), String> {
    let mut guard = REGISTRY.lock().map_err(|e| e.to_string())?;
    let list = guard.get_or_insert_with(Vec::new);
    for (name, doc) in entries {
        if list.iter().any(|e| e.name == *name) {
            return Err(format!("IPC 命令重复注册: {name}"));
        }
        list.push(IpcEntry { name, doc, owner });
    }
    Ok(())
}

/// 按命令名查归属者（插件路由的数据源；None = 未入库）
pub fn owner_of(command: &str) -> Option<&'static str> {
    REGISTRY.lock().ok().and_then(|g| {
        g.as_ref()
            .and_then(|list| list.iter().find(|e| e.name == command).map(|e| e.owner))
    })
}

/// 全量命令清单（framework_commands 命令的数据源）
pub fn snapshot() -> Vec<IpcEntry> {
    REGISTRY
        .lock()
        .map(|g| g.as_ref().cloned().unwrap_or_default())
        .unwrap_or_default()
}

/// 命令清单条目（framework_commands 的返回项）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcCommandInfo {
    /// 命令名
    name: String,
    /// 中文用途说明
    doc: String,
}

/// 框架命令：查询全量已入库命令（名称 + 说明）
#[tauri::command]
pub fn framework_commands() -> Vec<IpcCommandInfo> {
    snapshot()
        .into_iter()
        .map(|e| IpcCommandInfo {
            name: e.name.to_string(),
            doc: e.doc.to_string(),
        })
        .collect()
}

//! IPC 接口入库 · 命令注册表（规则见 docs/03-plugin-development.md §2）
//! - 插件 register() 时登记命令（名称 + 中文说明）
//! - 启动时校验全局唯一：重复注册直接 panic（开发期暴露冲突）
//! - 框架命令 framework_commands 返回全量清单（名称 + 说明），供调试/文档生成

use std::sync::Mutex;

/// 命令条目（入库最小单位）
#[derive(Clone)]
pub struct IpcEntry {
    /// 命令名（Tauri invoke 名，全局唯一）
    pub name: &'static str,
    /// 中文用途说明（供 framework_commands 查询与文档生成）
    pub doc: &'static str,
}

static REGISTRY: Mutex<Option<Vec<IpcEntry>>> = Mutex::new(None);

/// 插件 register() 调用：登记本插件命令清单
/// 返回 Err = 命令名冲突（重复注册），由调用方 panic 暴露
pub fn register(entries: &[(&'static str, &'static str)]) -> Result<(), String> {
    let mut guard = REGISTRY.lock().map_err(|e| e.to_string())?;
    let list = guard.get_or_insert_with(Vec::new);
    for (name, doc) in entries {
        if list.iter().any(|e| e.name == *name) {
            return Err(format!("IPC 命令重复注册: {name}"));
        }
        list.push(IpcEntry { name, doc });
    }
    Ok(())
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

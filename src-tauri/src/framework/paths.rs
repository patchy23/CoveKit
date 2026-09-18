//! 框架 · 统一存储路径（细则见 docs/standards/02-架构.md 的存储布局与位置迁移）
//!
//! 设计要点：
//! - **单一入口**：所有落盘路径必须经本模块解析，禁止插件手拼 `app_data_dir()`。
//! - **取值来自描述符**：四个分区（`data` / `vault` / `logs` / `cache`）一律取自
//!   `context` 固定下来的存储位置描述符（`StorageLocation`），本模块不做二次拼接；
//!   描述符有两种形态（旧扁平 / 分区），见 `context::LayoutKind`。
//! - **设备级与空间级分清**：`storage_root` 是**设备级**根（日志缓存分层基、根迁移源，
//!   跨空间共享）；空间内的数据与凭证取描述符的 `data` / `vault` 字段。
//! - **配置位置**：`settings.json` 的 `app.storageRoot`（空串 = 默认 `app_data_dir`）。
//!   配置类根下文件（`settings.json` / `covekit.json` / `.window-state.json`）永不搬移，
//!   因此配置的读取位置是常量，与数据根目录指向哪个盘无关（自举安全，见任务书 §3.1）。
//! - **老布局迁移**：根下的 `*.db`、`vault.dat`、`credentials/`、`ssh-known-hosts`、`tts/`、
//!   `agents/` 在启动早期一次性搬入四分区（同卷 rename 优先，跨卷退化为复制 + 校验 + 删源），
//!   迁移完成写 `layoutVersion` 防重放。
//! - **失败不丢数据**：迁移失败或未迁移时，`data_path` 会回落旧位置，升级后数据不会「消失」。

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::framework::context::StorageLocation;

/// 存储根目录配置键（位于 `settings.json` 的 `app` 对象内）
pub const KEY_STORAGE_ROOT: &str = "storageRoot";

/// 新建导入、存储迁移暂存目录共用的应用前缀。
pub const STAGING_PREFIX: &str = ".covekit-staging-";

/// 读取设置项（settings.json → app.<key>；不存在返回 None）
pub(crate) fn read_setting(app: &AppHandle, key: &str) -> Option<serde_json::Value> {
    let store = app.store("settings.json").ok()?;
    store.get("app")?.get(key).cloned()
}

/// 写入设置项（settings.json → app.<key>；失败返回错误）
/// 写入存储根目录配置（空字符串 = 恢复默认；重启后生效）。
///
/// 只有 `storage` 模块的「安排迁移 / 恢复动作」入口可以调用；通用设置写入会拒绝该键。
pub(crate) fn write_setting(
    app: &AppHandle,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut current = store.get("app").unwrap_or_else(|| serde_json::json!({}));
    if let serde_json::Value::Object(map) = &mut current {
        map.insert(key.to_string(), value);
    }
    store.set("app", current);
    store.save().map_err(|e| e.to_string())
}

/// 写入存储根目录配置（空字符串 = 恢复默认；重启后生效）
pub fn set_storage_root(app: &AppHandle, root: &str) -> Result<(), String> {
    write_setting(app, KEY_STORAGE_ROOT, serde_json::json!(root))
}

/// 计算的生效根目录（纯函数）：配置为空、非绝对路径 → 回退默认目录
pub fn resolve_root(configured: &str, default_root: &Path) -> PathBuf {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return default_root.to_path_buf();
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        candidate
    } else {
        default_root.to_path_buf()
    }
}

/// 目录可写性探针：尝试建立目录并写入临时文件后删除
pub fn is_writable_dir(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".covekit-write-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 默认根目录（平台 app_data_dir）
pub fn default_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("数据目录获取失败: {e}"))
}

/// 删除配置键（供存储计划等一次性状态使用；缺失即视为已删除）
pub(crate) fn remove_setting(app: &AppHandle, key: &str) -> Result<(), String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("打开设置存储失败: {e}"))?;
    let mut app_obj = store
        .get("app")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    app_obj.remove(key);
    store.set("app", serde_json::Value::Object(app_obj));
    store.save().map_err(|e| format!("保存设置失败: {e}"))
}

/// 配置里的存储根目录（未配置或空字符串时为 None）
pub fn configured_root(app: &AppHandle) -> Option<String> {
    read_setting(app, KEY_STORAGE_ROOT)
        .and_then(|v| v.as_str().map(String::from))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 当前生效的存储根目录（**设备级根**）。
///
/// 取值只走固定下来的数据上下文（`context::init_from_app` 在启动维护阶段之后解析一次）：
/// 运行期改配置不再中途换根，也不会每次调用重读 settings.json 或探测可写性。
/// 配置根不可用**不再**降级到默认目录：由 `context::init_from_app` 登记可见恢复状态，
/// 生效根保持配置值，业务读写失败可见，不会静默新建一套空环境。
/// 上下文尚未初始化时（单元测试或框架极早期）退回一次即时解析。
///
/// 语义提醒：本函数返回**设备级**根（日志缓存分层基与根迁移源，跨空间共享）。
/// 空间内的数据与凭证分区请经 `location_now` 取描述符字段，不要在此之上拼 `data` / `vault`。
pub fn storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(root) = crate::framework::context::root() {
        return Ok(root.to_path_buf());
    }
    resolve_root_now(app)
}

/// 本次调用使用的存储位置描述符。
///
/// 只借走固定上下文里的那一份（一次运行只解析一次，是数据位置的唯一来源）；
/// 上下文尚未初始化（单元测试或框架极早期）**即报错**：空间化之后没有任何
/// 「不经过上下文也能猜到数据位置」的合法路径，猜了就是往错误的地方写。
pub(crate) fn current_location(app: &AppHandle) -> Result<Cow<'static, StorageLocation>, String> {
    let _ = app;
    crate::framework::context::location()
        .map(Cow::Borrowed)
        .ok_or_else(|| "数据上下文未初始化，存储位置不可得".to_string())
}

/// 即时解析存储根（无上下文时的回落路径）：配置优先，不做可写性探测与降级
fn resolve_root_now(app: &AppHandle) -> Result<PathBuf, String> {
    let default = default_root(app)?;
    let configured = configured_root(app).unwrap_or_default();
    Ok(resolve_root(&configured, &default))
}

/// 校验作用域名（单段目录名，防路径穿越）
pub fn valid_scope(scope: &str) -> bool {
    !scope.is_empty()
        && !scope.contains('/')
        && !scope.contains('\\')
        && !scope.contains("..")
        && scope != "."
}

/// 分区名 → 分区目录（纯函数：四个分区一律取自描述符字段，不做二次拼接）
pub(crate) fn partition_path(location: &StorageLocation, name: &str) -> Option<PathBuf> {
    match name {
        "data" => Some(location.data.clone()),
        "vault" => Some(location.vault.clone()),
        "logs" => Some(location.logs.clone()),
        "cache" => Some(location.cache.clone()),
        _ => None,
    }
}

/// 分区目录（自动创建）：data / vault / logs / cache，取值一律来自存储位置描述符
pub fn partition_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !valid_scope(name) {
        return Err(format!("分区名非法: {name}"));
    }
    let location = current_location(app)?;
    // 四个分区之外的自定义目录落在空间根下（旧扁平布局下与设备级根相同）
    let dir = partition_path(&location, name).unwrap_or_else(|| location.root.join(name));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    Ok(dir)
}

/// 数据分区 `<空间根>/data`（插件 SQLite、known_hosts、本地凭据文件）
pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    partition_dir(app, "data")
}

// 日志分区根目录：SSH 会话日志按作用域取目录（`logs_dir_for`），根目录仅为统一入口的完整性保留。
#[allow(dead_code)]
/// 日志分区（旧扁平布局 `<设备根>/logs`；分区布局 `<设备根>/logs/<空间 id>`）
pub fn logs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    partition_dir(app, "logs")
}

/// 带作用域的日志目录（如 ssh）
pub fn logs_dir_for(app: &AppHandle, scope: &str) -> Result<PathBuf, String> {
    scoped_dir(app, "logs", scope)
}

/// 带作用域的缓存目录（如 agents / tts）
pub fn cache_dir(app: &AppHandle, scope: &str) -> Result<PathBuf, String> {
    scoped_dir(app, "cache", scope)
}

/// 带作用域的分区子目录（自动创建）
fn scoped_dir(app: &AppHandle, partition: &str, scope: &str) -> Result<PathBuf, String> {
    if !valid_scope(scope) {
        return Err(format!("作用域名非法: {scope}"));
    }
    let dir = partition_dir(app, partition)?.join(scope);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    Ok(dir)
}

/// 数据分区下某项的路径（只认本次生效的空间布局，不做旧布局回落）。
///
/// 旧布局的读取责任由启动维护窗口的迁移承担：迁移成功则旧位置已清空，迁移失败则启动
/// 进入恢复状态——两种情况下这里都没有「回设备根找旧文件」的合法场景。
pub fn data_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !valid_scope(name) {
        return Err(format!("文件名非法: {name}"));
    }
    let location = current_location(app)?;
    Ok(location.data.join(name))
}

/// 需要授权给资源协议（asset://）的目录清单
///
/// 只授权可播放产物所在目录（缓存分区下的 `tts`）：其他分区（data / vault / 布局文件、设置文件）
/// 一律不得通过 asset URL 访问，避免把整个数据目录暴露给 WebView。
/// 清单跟随本次生效的位置，因此自定义存储位置或空间切换后播放仍然可用。
pub fn asset_scope_dirs(location: &StorageLocation) -> Vec<PathBuf> {
    vec![location.cache.join("tts")]
}

/// 启动时把资源协议范围收敛到本次生效位置下的可播放目录
///
/// 拿不到生效位置时**不授权**（宁可不播放，也不放宽到任意磁盘）。
pub fn grant_asset_scope(app: &AppHandle) {
    let location = match current_location(app) {
        Ok(location) => location,
        Err(error) => {
            eprintln!("[asset] 未取到存储位置，跳过资源协议授权: {error}");
            return;
        }
    };
    let scope = app.asset_protocol_scope();
    for dir in asset_scope_dirs(&location) {
        if let Err(error) = scope.allow_directory(&dir, true) {
            eprintln!("[asset] 授权目录失败（{}）: {error}", dir.display());
        }
    }
}

/// 当前时间戳字符串（归档名用；本地时间 yyyymmddHHMMSS）
///
/// 仅用于生成人类可读的归档后缀，不参与任何判定，因此不要求时钟单调。
pub fn now_stamp() -> String {
    chrono::Local::now().format("%Y%m%d%H%M%S").to_string()
}

/// 路径总字节数（文件返回自身大小；目录递归累加；不存在返回 0）
pub fn dir_size(path: &Path) -> Result<u64, String> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(0),
    };
    if meta.is_file() {
        return Ok(meta.len());
    }
    let mut total = 0u64;
    let entries =
        std::fs::read_dir(path).map_err(|e| format!("读取目录 {} 失败: {e}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录 {} 项失败: {e}", path.display()))?;
        total += dir_size(&entry.path())?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建立本测试专用临时目录（避免并行用例互相干扰）
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("covekit-paths-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_root_falls_back_for_empty_or_relative() {
        let default = PathBuf::from("C:/default-root");
        assert_eq!(resolve_root("", &default), default);
        assert_eq!(resolve_root("   ", &default), default);
        assert_eq!(resolve_root("relative/dir", &default), default);
        // 绝对路径被采纳（Windows 与 POSIX 各测一个形态）
        let absolute = if cfg!(windows) {
            "D:/data/box"
        } else {
            "/data/box"
        };
        assert_eq!(resolve_root(absolute, &default), PathBuf::from(absolute));
    }

    #[test]
    fn valid_scope_rejects_path_traversal() {
        assert!(valid_scope("ssh"));
        assert!(valid_scope("tts"));
        assert!(!valid_scope(""));
        assert!(!valid_scope(".."));
        assert!(!valid_scope("../evil"));
        assert!(!valid_scope("a/b"));
        assert!(!valid_scope("a\\b"));
        assert!(!valid_scope("."));
    }

    #[test]
    fn is_writable_dir_creates_and_probes() {
        let dir = temp_dir("writable");
        assert!(is_writable_dir(&dir.join("nested")));
        // 探针文件不残留
        assert!(!dir.join("nested").join(".covekit-write-probe").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dir_size_counts_files_recursively() {
        let dir = temp_dir("size");
        std::fs::write(dir.join("a"), b"1234").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("b"), b"12345").unwrap();
        assert_eq!(dir_size(&dir).unwrap(), 9);
        assert_eq!(dir_size(&dir.join("missing")).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 四分区取值只来自描述符字段：与描述符一致，且不二次拼接
    #[test]
    fn partition_path_follows_descriptor_fields() {
        let location = StorageLocation::for_space(
            PathBuf::from("D:/pb-root"),
            "9f1c4e2a-1111-4111-8111-111111111111",
        );
        assert_eq!(partition_path(&location, "data").unwrap(), location.data);
        assert_eq!(partition_path(&location, "vault").unwrap(), location.vault);
        assert_eq!(partition_path(&location, "logs").unwrap(), location.logs);
        assert_eq!(partition_path(&location, "cache").unwrap(), location.cache);
        // 四个分区之外的名字不映射到分区（由调用方决定落位）
        assert!(partition_path(&location, "spaces").is_none());
        assert_eq!(
            partition_path(&location, "logs").unwrap(),
            PathBuf::from("D:/pb-root/logs/9f1c4e2a-1111-4111-8111-111111111111")
        );
    }

    /// 资源协议只授权缓存分区下的可播放目录（不得因为空间变化而放宽到整个数据目录）
    #[test]
    fn asset_scope_stays_inside_cache_partition() {
        let location = StorageLocation::for_space(
            PathBuf::from("D:/pb-root"),
            "9f1c4e2a-1111-4111-8111-111111111111",
        );
        let dirs = asset_scope_dirs(&location);
        assert_eq!(
            dirs,
            vec![PathBuf::from(
                "D:/pb-root/cache/9f1c4e2a-1111-4111-8111-111111111111/tts"
            )]
        );
        for dir in dirs {
            assert!(!dir.starts_with(&location.data), "不得授权数据分区");
            assert!(!dir.starts_with(&location.vault), "不得授权凭证分区");
        }
    }
}

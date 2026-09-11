//! 框架 · 统一存储路径（细则 `docs/tasks/2026-09-11-存储目录配置任务书.md`）
//!
//! 设计要点：
//! - **单一入口**：所有落盘路径必须经本模块解析，禁止插件手拼 `app_data_dir()`。
//! - **四分区**：`<root>/data`（插件数据库与本地文件）、`<root>/vault`（凭证密文与降级密钥）、
//!   `<root>/logs/<scope>`（日志）、`<root>/cache/<scope>`（可重建缓存）。
//! - **配置位置**：`settings.json` 的 `app.storageRoot`（空串 = 默认 `app_data_dir`）。
//!   配置类根下文件（`settings.json` / `patchybox.json` / `.window-state.json`）永不搬移，
//!   因此配置的读取位置是常量，与数据根目录指向哪个盘无关（自举安全，见任务书 §3.1）。
//! - **老布局迁移**：根下的 `*.db`、`vault.dat`、`credentials/`、`ssh-known-hosts`、`tts/`、
//!   `agents/` 在启动早期一次性搬入四分区（同卷 rename 优先，跨卷退化为复制 + 校验 + 删源），
//!   迁移完成写 `layoutVersion` 防重放。
//! - **失败不丢数据**：迁移失败或未迁移时，`data_path` 会回落旧位置，升级后数据不会「消失」。

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

/// 存储根目录配置键（位于 `settings.json` 的 `app` 对象内）
pub const KEY_STORAGE_ROOT: &str = "storageRoot";
/// 布局版本键（记录四分区布局迁移是否已完成）
pub const KEY_LAYOUT_VERSION: &str = "layoutVersion";
/// 当前布局版本（1 = 四分区布局）
pub const LAYOUT_VERSION: i64 = 1;

/// 读取设置项（settings.json → app.<key>；不存在返回 None）
fn read_setting(app: &AppHandle, key: &str) -> Option<serde_json::Value> {
    let store = app.store("settings.json").ok()?;
    store.get("app")?.get(key).cloned()
}

/// 写入设置项（settings.json → app.<key>；失败返回错误）
fn write_setting(app: &AppHandle, key: &str, value: serde_json::Value) -> Result<(), String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut current = store.get("app").unwrap_or_else(|| serde_json::json!({}));
    if let serde_json::Value::Object(map) = &mut current {
        map.insert(key.to_string(), value);
    }
    store.set("app", current);
    store.save().map_err(|e| e.to_string())
}

/// 读取已完成的布局版本（0 = 尚未执行四分区迁移）
pub fn layout_version(app: &AppHandle) -> i64 {
    read_setting(app, KEY_LAYOUT_VERSION)
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
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
    let probe = dir.join(".patchybox-write-probe");
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

/// 当前生效的存储根目录；配置的目录不可用（拔盘/只读/无权限）时降级默认目录并告警
pub fn storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    let default = default_root(app)?;
    let configured = read_setting(app, KEY_STORAGE_ROOT)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let root = resolve_root(&configured, &default);
    if root != default && !is_writable_dir(&root) {
        eprintln!(
            "[storage] 配置的存储目录不可用，已降级到默认目录: {}",
            root.display()
        );
        return Ok(default);
    }
    Ok(root)
}

/// 校验作用域名（单段目录名，防路径穿越）
pub fn valid_scope(scope: &str) -> bool {
    !scope.is_empty()
        && !scope.contains('/')
        && !scope.contains('\\')
        && !scope.contains("..")
        && scope != "."
}

/// 分区目录（自动创建）：data / logs / vault
pub fn partition_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !valid_scope(name) {
        return Err(format!("分区名非法: {name}"));
    }
    let dir = storage_root(app)?.join(name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    Ok(dir)
}

/// 数据分区 `<root>/data`（插件 SQLite、known_hosts、本地凭据文件）
pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    partition_dir(app, "data")
}

/// 凭证分区 `<root>/vault`
pub fn vault_dir(app: &AppHandle) -> Result<PathBuf, String> {
    partition_dir(app, "vault")
}

/// 日志根目录 `<root>/logs`
// 会话日志（SSH）等日志消费方接入后即被使用；在此之前仅为统一入口的完整性保留。
#[allow(dead_code)]
pub fn logs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    partition_dir(app, "logs")
}

/// 带作用域的日志目录 `<root>/logs/<scope>`（如 ssh）
// 供 SSH 会话日志落盘使用（见 docs/plugins/ssh/2026-09-11-扩展任务书.md 第 4 项）。
#[allow(dead_code)]
pub fn logs_dir_for(app: &AppHandle, scope: &str) -> Result<PathBuf, String> {
    scoped_dir(app, "logs", scope)
}

/// 带作用域的缓存目录 `<root>/cache/<scope>`（如 agents / tts）
pub fn cache_dir(app: &AppHandle, scope: &str) -> Result<PathBuf, String> {
    scoped_dir(app, "cache", scope)
}

/// 带作用域的分区子目录（自动创建）
fn scoped_dir(app: &AppHandle, partition: &str, scope: &str) -> Result<PathBuf, String> {
    if !valid_scope(scope) {
        return Err(format!("作用域名非法: {scope}"));
    }
    let dir = storage_root(app)?.join(partition).join(scope);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    Ok(dir)
}

/// 数据分区下某项的路径（**带旧布局回落**）。
///
/// 新位置（`data/<name>`）不存在而根下旧位置存在时返回旧位置：布局迁移失败或被跳过的
/// 极端情况下仍能读到老数据，避免「升级后数据消失」。
pub fn data_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    if !valid_scope(name) {
        return Err(format!("文件名非法: {name}"));
    }
    let root = storage_root(app)?;
    let dir = data_dir(app)?;
    let target = dir.join(name);
    let legacy = root.join(name);
    if !target.exists() && legacy.exists() {
        return Ok(legacy);
    }
    Ok(target)
}

// ──────────────────────────────────────────────────────────────────────────
// 老布局一次性迁移
// ──────────────────────────────────────────────────────────────────────────

/// 迁移计划的固定项：`(根下旧名, 目标分区)`；`*.db` 由扫描补充
const FIXED_MOVES: [(&str, &str); 7] = [
    ("ssh-known-hosts", "data"),
    ("credentials", "data"),
    ("credentials-master.key", "data"),
    ("vault.dat", "vault"),
    ("vault-master.key", "vault"),
    ("tts", "cache"),
    ("agents", "cache"),
];

/// 老布局 → 四分区迁移（幂等；已完成则直接返回）。
/// 调用时机：`lib.rs` 的 setup 中**最先**执行，早于任何插件打开数据库。
pub fn migrate_layout(app: &AppHandle) -> Result<(), String> {
    let done = read_setting(app, KEY_LAYOUT_VERSION)
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if done >= LAYOUT_VERSION {
        return Ok(());
    }
    let root = storage_root(app)?;

    let mut moved = 0usize;
    // 固定项
    for (name, partition) in FIXED_MOVES {
        let from = root.join(name);
        let to = root.join(partition).join(name);
        match move_path(&from, &to) {
            Ok(true) => moved += 1,
            Ok(false) => {}
            Err(e) => eprintln!("[storage] 迁移 {name} 失败（保留原位置）: {e}"),
        }
    }
    // 根下插件数据库 *.db
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("db") {
                continue;
            }
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let to = root.join("data").join(file_name);
            match move_path(&path, &to) {
                Ok(true) => moved += 1,
                Ok(false) => {}
                Err(e) => eprintln!("[storage] 迁移 {} 失败（保留原位置）: {e}", path.display()),
            }
        }
    }

    write_setting(app, KEY_LAYOUT_VERSION, serde_json::json!(LAYOUT_VERSION))?;
    if moved > 0 {
        eprintln!("[storage] 已完成布局迁移：{moved} 项移入 data/vault/cache 分区");
    }
    Ok(())
}

/// 移动单一路径（文件或目录）。返回是否真的搬了。
/// - 源不存在或目标已存在 → 跳过（绝不覆盖目标，避免破坏已有数据）
/// - 同卷 rename 优先；跨卷失败退化为复制 + 大小校验 + 删源
pub fn move_path(from: &Path, to: &Path) -> Result<bool, String> {
    if !from.exists() || to.exists() {
        return Ok(false);
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(true);
    }
    // 跨卷/被占用：复制 → 校验 → 删源
    copy_recursive(from, to)?;
    if dir_size(from)? != dir_size(to)? {
        return Err("复制体积不一致，已放弃删源".into());
    }
    remove_recursive(from)?;
    Ok(true)
}

/// 递归复制（文件或目录）
pub fn copy_recursive(from: &Path, to: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(from).map_err(|e| format!("读取 {} 失败: {e}", from.display()))?;
    if meta.is_file() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from, to).map_err(|e| format!("复制 {} 失败: {e}", from.display()))?;
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from)
        .map_err(|e| format!("读取目录 {} 失败: {e}", from.display()))?
        .flatten()
    {
        let name = entry.file_name();
        copy_recursive(&entry.path(), &to.join(name))?;
    }
    Ok(())
}

/// 递归删除（仅用于跨卷迁移成功后的源清理）
pub fn remove_recursive(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("删除 {} 失败: {e}", path.display()))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("删除 {} 失败: {e}", path.display()))
    }
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
    for entry in std::fs::read_dir(path)
        .map_err(|e| format!("读取目录 {} 失败: {e}", path.display()))?
        .flatten()
    {
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
            std::env::temp_dir().join(format!("patchybox-paths-test-{tag}-{}", std::process::id()));
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
        assert!(!dir.join("nested").join(".patchybox-write-probe").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_path_moves_file_and_skips_existing_target() {
        let dir = temp_dir("move");
        let from = dir.join("root-level.db");
        let to = dir.join("data").join("root-level.db");
        std::fs::write(&from, b"payload").unwrap();
        assert!(move_path(&from, &to).unwrap());
        assert!(!from.exists());
        assert_eq!(std::fs::read(&to).unwrap(), b"payload");

        // 目标已存在时不覆盖
        let again = dir.join("again.db");
        std::fs::write(&again, b"new").unwrap();
        assert!(!move_path(&again, &to).unwrap());
        assert_eq!(std::fs::read(&to).unwrap(), b"payload");
        assert!(again.exists());

        // 源不存在时也不报错
        assert!(!move_path(&dir.join("missing.db"), &to).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_path_moves_directory_tree() {
        let dir = temp_dir("move-dir");
        let src = dir.join("credentials");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("ssh.enc"), b"abc").unwrap();
        std::fs::write(src.join("nested").join("x.enc"), b"de").unwrap();
        let dst = dir.join("data").join("credentials");
        assert!(move_path(&src, &dst).unwrap());
        assert!(!src.exists());
        assert_eq!(std::fs::read(dst.join("ssh.enc")).unwrap(), b"abc");
        assert!(dst.join("nested").join("x.enc").exists());
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
}

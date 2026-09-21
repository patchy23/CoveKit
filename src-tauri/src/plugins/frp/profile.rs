//! frp 插件 · 配置档文件能力（路径安全 / 扫描 / 读写 / 新建 / 复制 / 重命名 / 软删）
//! 安全红线（任务书 §5.1）：`fileName` 一律视为单段文件名，拒绝路径分隔符、上级引用、绝对路径、
//! Windows 保留名与非 `.toml` 扩展名；读写前用 canonicalize 确认最终路径仍在配置目录内，绝不 panic。
//! 分层：本模块只做「文件系统能力」（不碰 AppHandle / DB / 响应结构体），装配与响应拼装在门面 `mod.rs`。

use std::path::{Path, PathBuf};

/// 档案扩展名（强制 `.toml`；比较时不区分大小写）
const PROFILE_EXT: &str = ".toml";
/// 软删除目录名（配置目录内；删除即移入，不物理抹除）
const TRASH_DIR: &str = ".trash";
/// 备份保留份数上限（每次写入前生成，超出的最旧备份随写入清理）
const MAX_BACKUPS: usize = 5;
/// 文件名长度上限（字节；防超长名撑爆路径）
const MAX_NAME_LEN: usize = 128;
/// Windows 保留设备名（stem 命中即拒，如 `CON.toml` / `nul.toml`）
const RESERVED_STEMS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/* ── 路径安全（纯函数 + 目录规范化） ── */

/// 校验档案文件名（纯函数）：只接受「单段 + `.toml` 扩展名」的文件名，违反即返回中文原因
pub fn validate_file_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("文件名不能为空".into());
    }
    if trimmed.len() > MAX_NAME_LEN {
        return Err(format!("文件名过长（最多 {MAX_NAME_LEN} 字节）"));
    }
    // 单段文件名：路径分隔符与上级引用直接拒绝（防目录逃逸）
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err("非法文件名：只能是不含路径分隔符的单段文件名".into());
    }
    // 盘符/冒号（绝对路径）与文件名非法字符
    if trimmed.contains(':')
        || trimmed
            .chars()
            .any(|c| c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err("非法文件名：含系统不允许的字符（: * ? \" < > |）".into());
    }
    if !trimmed.to_ascii_lowercase().ends_with(PROFILE_EXT) {
        return Err("非法文件名：仅支持 .toml 配置档".into());
    }
    // 已确认 ASCII 后缀，按字节切片安全
    let stem = &trimmed[..trimmed.len() - PROFILE_EXT.len()];
    if stem.is_empty() {
        return Err("非法文件名：缺少主文件名".into());
    }
    if trimmed.ends_with('.') || trimmed.ends_with(' ') {
        return Err("非法文件名：不能以点或空格结尾".into());
    }
    if stem.starts_with('.') {
        return Err("非法文件名：不允许隐藏文件".into());
    }
    if RESERVED_STEMS.contains(&stem.to_ascii_lowercase().as_str()) {
        return Err(format!("非法文件名：{stem} 是系统保留名"));
    }
    Ok(trimmed)
}

/// 确保配置目录存在并返回其规范化绝对路径（后续所有路径校验的基准）
pub(crate) async fn ensure_dir(dir: &Path) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("创建配置目录失败（{}）: {e}", dir.display()))?;
    tokio::fs::canonicalize(dir)
        .await
        .map_err(|e| format!("解析配置目录失败: {e}"))
}

/// 解析档案绝对路径：先校验文件名，再用 canonicalize 确认真实路径仍在配置目录内（防符号链接逃逸）
pub(crate) async fn resolve_profile_path(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let valid = validate_file_name(name)?;
    let root = ensure_dir(dir).await?;
    let path = root.join(valid);
    if path.exists() {
        let real = tokio::fs::canonicalize(&path)
            .await
            .map_err(|e| format!("解析档案路径失败: {e}"))?;
        if real.parent() != Some(root.as_path()) {
            return Err("非法文件名：目标不在配置目录内".into());
        }
        return Ok(real);
    }
    Ok(path)
}

/// 要求档案已存在，返回其绝对路径（不存在即返回中文原因）
pub(crate) async fn require_profile(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let path = resolve_profile_path(dir, name).await?;
    if !path.exists() {
        return Err(format!("档案 {name} 不存在"));
    }
    Ok(path)
}

/* ── 扫描与读写 ── */

/// 扫描配置目录下的 `*.toml`（跳过子目录、隐藏文件与 `.trash`；按文件名排序）
pub(crate) async fn list_profile_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let root = ensure_dir(dir).await?;
    let mut paths = Vec::new();
    let mut entries = tokio::fs::read_dir(&root)
        .await
        .map_err(|e| format!("读取配置目录失败: {e}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("读取配置目录失败: {e}"))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        // 只认单段非隐藏的 .toml 文件（.trash 目录名以点开头，天然被排除）
        if name.starts_with('.') || !name.to_ascii_lowercase().ends_with(PROFILE_EXT) {
            continue;
        }
        if !entry
            .file_type()
            .await
            .map_err(|e| e.to_string())?
            .is_file()
        {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

/// 读取档案原文（原样返回，不做任何改写）
pub(crate) async fn read_profile_text(dir: &Path, name: &str) -> Result<String, String> {
    let path = require_profile(dir, name).await?;
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("读取档案失败: {e}"))
}

/// 写入档案原文（写前自动生成 `.bak-<ts>` 备份，并只保留最近 MAX_BACKUPS 份）
pub(crate) async fn write_profile_text(
    dir: &Path,
    name: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let path = resolve_profile_path(dir, name).await?;
    if path.exists() {
        backup_file(&path).await?;
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("写入档案失败: {e}"))?;
    rotate_backups(dir, name).await;
    Ok(path)
}

/// 备份单个档案（配置目录内 `<name>.toml.bak-<毫秒时间戳>`）
async fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .ok_or("档案路径缺少文件名")?
        .to_string_lossy()
        .to_string();
    let backup = path.with_file_name(format!(
        "{file_name}.bak-{}",
        chrono::Utc::now().timestamp_millis()
    ));
    tokio::fs::copy(path, &backup)
        .await
        .map_err(|e| format!("备份档案失败: {e}"))?;
    Ok(backup)
}

/// 清理旧备份（定长毫秒时间戳 → 字典序即时间序；保留最近 MAX_BACKUPS 份，失败仅告警不阻断写入）
pub(crate) async fn rotate_backups(dir: &Path, name: &str) {
    let prefix = format!("{name}.bak-");
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let mut backups = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            backups.push(entry.path());
        }
    }
    backups.sort();
    for stale in backups.into_iter().rev().skip(MAX_BACKUPS) {
        if let Err(e) = tokio::fs::remove_file(&stale).await {
            log::warn!(
                "旧备份清理失败 error_type={}",
                std::any::type_name_of_val(&e)
            );
        }
    }
}

/* ── 新建 / 复制 / 重命名 / 软删 ── */

/// 内置模板内容（纯函数；`tcp` / `http` / `stcp` / `empty`，均为可被 `frpc verify` 通过的合法 TOML）
pub fn template_content(template: &str) -> Result<String, String> {
    let base = "serverAddr = \"127.0.0.1\"\nserverPort = 7000\n\n[auth]\nmethod = \"token\"\ntoken = \"请填写与服务端一致的 token\"\n\n";
    match template {
        "tcp" => Ok(format!("{base}[[proxies]]\nname = \"example-tcp\"\ntype = \"tcp\"\nlocalIP = \"127.0.0.1\"\nlocalPort = 8080\nremotePort = 16080\nenabled = true\n")),
        "http" => Ok(format!("{base}[[proxies]]\nname = \"example-http\"\ntype = \"http\"\nlocalIP = \"127.0.0.1\"\nlocalPort = 8080\ncustomDomains = [\"example.example.com\"]\nenabled = true\n")),
        "stcp" => Ok(format!("{base}[[proxies]]\nname = \"example-stcp\"\ntype = \"stcp\"\nlocalIP = \"127.0.0.1\"\nlocalPort = 8080\nsecretKey = \"请填写访问密钥\"\nenabled = true\n")),
        "empty" => Ok("serverAddr = \"127.0.0.1\"\nserverPort = 7000\n".to_string()),
        _ => Err(format!("不支持的模板: {template}")),
    }
}

/// 新建档案（模板内容直接落盘；目标已存在即拒绝）
pub(crate) async fn create_profile(
    dir: &Path,
    name: &str,
    template: &str,
) -> Result<PathBuf, String> {
    let content = template_content(template)?;
    let path = resolve_profile_path(dir, name).await?;
    if path.exists() {
        return Err(format!("档案 {name} 已存在，请换一个名字"));
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("新建档案失败: {e}"))?;
    Ok(path)
}

/// 解析「源 + 目标」路径对：源必须存在、目标必须不存在（重命名与复制的共用前置检查）
async fn resolve_pair(
    dir: &Path,
    name: &str,
    new_name: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let source = require_profile(dir, name).await?;
    let target = resolve_profile_path(dir, new_name).await?;
    if target.exists() {
        return Err(format!("档案 {new_name} 已存在，请换一个名字（不覆盖）"));
    }
    Ok((source, target))
}

/// 复制档案（保留原文；目标必须不存在）
pub(crate) async fn duplicate_profile(
    dir: &Path,
    name: &str,
    new_name: &str,
) -> Result<PathBuf, String> {
    let (source, target) = resolve_pair(dir, name, new_name).await?;
    tokio::fs::copy(&source, &target)
        .await
        .map_err(|e| format!("复制档案失败: {e}"))?;
    Ok(target)
}

/// 重命名档案（冲突即拒绝，绝不覆盖同名档案）
pub(crate) async fn rename_profile(
    dir: &Path,
    name: &str,
    new_name: &str,
) -> Result<PathBuf, String> {
    let (source, target) = resolve_pair(dir, name, new_name).await?;
    tokio::fs::rename(&source, &target)
        .await
        .map_err(|e| format!("重命名失败: {e}"))?;
    Ok(target)
}

/// 软删除档案（移入配置目录 `.trash/<name>.<ts>`，不做物理抹除）
pub(crate) async fn delete_profile(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let source = require_profile(dir, name).await?;
    let trash = ensure_dir(dir).await?.join(TRASH_DIR);
    tokio::fs::create_dir_all(&trash)
        .await
        .map_err(|e| format!("创建 .trash 目录失败: {e}"))?;
    let target = trash.join(format!("{name}.{}", chrono::Utc::now().timestamp_millis()));
    tokio::fs::rename(&source, &target)
        .await
        .map_err(|e| format!("移入 .trash 失败: {e}"))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用临时目录（标签 + 进程号命名，避免并行用例互相干扰）
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("covekit-frp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 统计目录下某档案的备份份数
    fn backup_count(dir: &Path, name: &str) -> usize {
        let prefix = format!("{name}.bak-");
        std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .map(|x| x.file_name().to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
            })
            .count()
    }

    #[test]
    fn illegal_file_names_are_all_rejected() {
        for bad in [
            "../x.toml",
            "a/b.toml",
            "a\\b.toml",
            "C:\\x.toml",
            "C:/x.toml",
            "CON.toml",
            "nul.toml",
            "",
            "   ",
            "a.txt",
            "x.toml.txt",
            ".toml",
            ".hidden.toml",
            "a*.toml",
            "a..toml",
        ] {
            assert!(validate_file_name(bad).is_err(), "应拒绝: {bad}");
        }
    }

    #[test]
    fn legal_file_names_pass() {
        for ok in [
            "web.toml",
            "内网 隧道.toml",
            "my-profile_2.toml",
            "TCP.TOML",
        ] {
            assert_eq!(validate_file_name(ok).unwrap(), ok.trim());
        }
    }

    #[test]
    fn templates_are_valid_toml() {
        for id in ["tcp", "http", "stcp", "empty"] {
            let text = template_content(id).unwrap();
            assert!(
                toml::from_str::<toml::Value>(&text).is_ok(),
                "模板 {id} 不是合法 TOML"
            );
        }
        assert!(template_content("nope").is_err());
    }

    #[tokio::test]
    async fn write_creates_backup_and_rotates_to_five() {
        let dir = temp_dir("backup");
        for i in 0..8 {
            write_profile_text(&dir, "svc.toml", &format!("serverPort = {i}\n"))
                .await
                .unwrap();
        }
        let text = tokio::fs::read_to_string(dir.join("svc.toml"))
            .await
            .unwrap();
        assert!(text.contains("serverPort = 7"), "最后一次写入应生效");
        let count = backup_count(&dir, "svc.toml");
        assert!(
            count > 0 && count <= MAX_BACKUPS,
            "备份应保留最近 {MAX_BACKUPS} 份，实际 {count}"
        );
    }

    #[tokio::test]
    async fn rename_and_duplicate_reject_conflict() {
        let dir = temp_dir("rename");
        create_profile(&dir, "a.toml", "tcp").await.unwrap();
        create_profile(&dir, "b.toml", "empty").await.unwrap();
        let err = rename_profile(&dir, "a.toml", "b.toml").await.unwrap_err();
        assert!(err.contains("已存在"), "应阻止覆盖同名档案: {err}");
        assert!(duplicate_profile(&dir, "b.toml", "a.toml").await.is_err());
        rename_profile(&dir, "a.toml", "c.toml").await.unwrap();
        assert!(dir.join("c.toml").exists() && !dir.join("a.toml").exists());
        // 源不存在 / 非法新名同样被拒
        assert!(rename_profile(&dir, "ghost.toml", "d.toml").await.is_err());
        assert!(rename_profile(&dir, "c.toml", "../evil.toml")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn delete_moves_file_into_trash() {
        let dir = temp_dir("trash");
        create_profile(&dir, "gone.toml", "http").await.unwrap();
        let target = delete_profile(&dir, "gone.toml").await.unwrap();
        let trash_root = ensure_dir(&dir).await.unwrap().join(TRASH_DIR);
        assert!(target.starts_with(&trash_root), "删除应移入 .trash");
        assert!(target.exists(), "删除不得物理抹除");
        assert!(!dir.join("gone.toml").exists());
        // 非法名不落盘、不存在名报错
        assert!(delete_profile(&dir, "../evil.toml").await.is_err());
        assert!(!dir.join("evil.toml").exists());
        assert!(read_profile_text(&dir, "ghost.toml").await.is_err());
    }

    #[tokio::test]
    async fn create_rejects_existing_and_illegal_name() {
        let dir = temp_dir("guard");
        let err = create_profile(&dir, "../evil.toml", "tcp")
            .await
            .unwrap_err();
        assert!(err.contains("非法文件名"), "越界文件名应被拒: {err}");
        assert!(!dir.join("evil.toml").exists());
        create_profile(&dir, "keep.toml", "empty").await.unwrap();
        assert!(create_profile(&dir, "keep.toml", "empty").await.is_err());
    }
}

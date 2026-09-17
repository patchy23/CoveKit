//! 适配器存储清单的安全路径与目录展开；快照、修订号及恢复共用。

use std::path::{Component, Path, PathBuf};

/// 只接受空间内的普通相对路径，拒绝链接和跨平台绝对路径。
pub(crate) fn resolve(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains(':')
        || relative.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("存储清单含非法相对路径".into());
    }
    let mut target = root.to_path_buf();
    for part in path.components() {
        target.push(part);
        match target.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("存储清单不能指向符号链接".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("检查存储路径失败: {e}")),
        }
    }
    Ok(target)
}

/// 递归展开明确授权的空间管理目录，不跟随链接。
pub(crate) fn expand(root: &Path, relative: &str) -> Result<Vec<String>, String> {
    let target = resolve(root, relative)?;
    let meta = match target.symlink_metadata() {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("读取存储元数据失败: {e}")),
    };
    if meta.is_file() {
        return Ok(vec![relative.into()]);
    }
    if !meta.is_dir() {
        return Err("不支持的存储文件类型".into());
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(target).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "存储文件名不是有效 Unicode")?;
        result.extend(expand(root, &format!("{relative}/{name}"))?);
    }
    result.sort();
    Ok(result)
}

/// 所有已注册存储及导入映射；同一偏好文件由多个框架适配器共享。
pub(crate) fn tracked() -> Result<Vec<String>, String> {
    let mut files = std::collections::BTreeSet::from([
        super::lineage::IMPORT_MAP_FILE.to_string(),
        "data/ssh.db".into(),
        "preferences.json".into(),
    ]);
    for adapter in super::adapter::all() {
        files.extend(adapter.storage_files());
    }
    for file in &files {
        resolve(Path::new("."), file)?;
    }
    Ok(files.into_iter().collect())
}

/// SQLite 在线复制，包含 WAL 已提交页；还原时不替换现有连接所持的文件。
pub(crate) fn copy_database(source: &Path, target: &Path) -> Result<(), String> {
    let source =
        rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
    let mut target = rusqlite::Connection::open(target).map_err(|e| e.to_string())?;
    target
        .busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|e| e.to_string())?;
    let backup = rusqlite::backup::Backup::new(&source, &mut target).map_err(|e| e.to_string())?;
    // 一次复制全部页；锁冲突可见失败，不无限重试等待外部进程。
    match backup.step(-1).map_err(|e| e.to_string())? {
        rusqlite::backup::StepResult::Done => Ok(()),
        _ => Err("数据库正在被其他操作占用，请稍后重试快照或恢复".into()),
    }
}

/// 根据实际文件头识别 SQLite；测试普通文件和非数据库文件仍原样复制。
pub(crate) fn is_database(path: &Path) -> Result<bool, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 16];
    let count = file.read(&mut header).map_err(|e| e.to_string())?;
    Ok(count == 16 && &header == b"SQLite format 3\0")
}

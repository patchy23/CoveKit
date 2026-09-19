//! 已删除配置的查询与恢复；只接受当前配置目录和空间导入目录中的回收站文件。

use super::models::FrpDeletedProfile as DeletedProfile;
use super::{metadata::op_result, models::FrpOpResult, profile, profile_dir, transfer};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

fn parse_name(name: &str) -> Result<(&str, i64), String> {
    let (original, stamp) = name.rsplit_once('.').ok_or("无效的回收站文件名")?;
    if profile::validate_file_name(original)? != original {
        return Err("无效的回收站文件名".into());
    }
    if stamp.is_empty() || !stamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("无效的删除时间".into());
    }
    Ok((original, stamp.parse().map_err(|_| "无效的删除时间")?))
}

async fn list_in(dir: &Path, managed: bool) -> Result<Vec<DeletedProfile>, String> {
    let mut entries = match tokio::fs::read_dir(dir.join(".trash")).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("读取已删除配置失败：{error}")),
    };
    let mut result = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        if !entry
            .file_type()
            .await
            .map_err(|error| error.to_string())?
            .is_file()
        {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        let Ok((original, deleted_at)) = parse_name(&name) else {
            continue;
        };
        result.push(DeletedProfile {
            file_name: original.into(),
            trash_name: name,
            deleted_at,
            managed,
        });
    }
    Ok(result)
}

/// 列出当前空间与配置目录内的已删除档案。
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profiles_deleted(app: AppHandle) -> Result<Vec<DeletedProfile>, String> {
    let dir = profile_dir(&app)?;
    let managed = transfer::managed_dir(&app)?;
    let mut result = list_in(&dir, false).await?;
    if managed != dir {
        result.extend(list_in(&managed, true).await?);
    }
    result.sort_by(|left, right| right.deleted_at.cmp(&left.deleted_at));
    Ok(result)
}

async fn restore_in(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let (original, _) = parse_name(name)?;
    let trash = dir.join(".trash");
    let source = trash.join(name);
    let metadata = tokio::fs::symlink_metadata(&source)
        .await
        .map_err(|error| format!("读取已删除配置失败：{error}"))?;
    if !metadata.is_file() {
        return Err("已删除配置不是普通文件".into());
    }
    let source = tokio::fs::canonicalize(source)
        .await
        .map_err(|error| error.to_string())?;
    let root = tokio::fs::canonicalize(dir)
        .await
        .map_err(|error| error.to_string())?;
    if source.parent() != Some(root.join(".trash").as_path()) {
        return Err("回收站路径不在配置目录内".into());
    }
    let content = tokio::fs::read(&source)
        .await
        .map_err(|error| error.to_string())?;
    let target = root.join(original);
    // create_new 保证并发写入或外部文件创建也不会被恢复操作覆盖。
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .await
        .map_err(|error| format!("无法恢复，目标可能已存在：{error}"))?;
    let written = async {
        output.write_all(&content).await?;
        output.sync_all().await
    }
    .await;
    drop(output);
    if let Err(error) = written {
        let cleanup = tokio::fs::remove_file(&target).await;
        return Err(format!(
            "恢复写入失败：{error}；清理未完成文件：{cleanup:?}"
        ));
    }
    tokio::fs::remove_file(&source)
        .await
        .map_err(|error| format!("配置已恢复，但回收站副本未能清理：{error}"))?;
    Ok(target)
}

/// 恢复到原目录和原文件名；跨可见目录检查重名，不自动启动。
#[tauri::command(rename_all = "camelCase")]
pub async fn frp_profile_restore(
    app: AppHandle,
    trash_name: String,
    managed: bool,
) -> Result<FrpOpResult, String> {
    let _maintenance = crate::framework::context::maintenance_guard().await;
    let (original, _) = parse_name(&trash_name)?;
    transfer::ensure_name_available(&app, original)?;
    let dir = if managed {
        transfer::managed_dir(&app)?
    } else {
        profile_dir(&app)?
    };
    let target = restore_in(&dir, &trash_name).await?;
    Ok(op_result(&target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn restore_preserves_content_and_refuses_overwrite() {
        let dir = std::env::temp_dir().join(format!("frp-restore-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(dir.join(".trash")).await.unwrap();
        let source = dir.join(".trash/test.toml.123");
        tokio::fs::write(&source, "# 原始注释\nserverPort = 7000\n")
            .await
            .unwrap();
        assert_eq!(list_in(&dir, false).await.unwrap().len(), 1);
        tokio::fs::write(dir.join("test.toml"), "existing")
            .await
            .unwrap();
        assert!(restore_in(&dir, "test.toml.123").await.is_err());
        assert_eq!(
            tokio::fs::read_to_string(dir.join("test.toml"))
                .await
                .unwrap(),
            "existing"
        );
        assert!(source.exists());
        tokio::fs::remove_file(dir.join("test.toml")).await.unwrap();
        restore_in(&dir, "test.toml.123").await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(dir.join("test.toml"))
                .await
                .unwrap(),
            "# 原始注释\nserverPort = 7000\n"
        );
        assert!(!source.exists());
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }

    #[test]
    fn rejects_invalid_trash_names() {
        for name in [
            "../a.toml.123",
            "a.toml/123",
            "a.toml.-1",
            "a.toml",
            "a.toml.123/x",
        ] {
            assert!(parse_name(name).is_err(), "{name}");
        }
    }
}

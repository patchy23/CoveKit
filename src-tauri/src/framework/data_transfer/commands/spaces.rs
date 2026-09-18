//! 数据传输命令 · spaces

use crate::framework::context::maintenance_guard;

use super::views::BackupSummary;
use super::views::SpaceSummary;
use super::views::SpaceSwitchResult;
use crate::framework::settings::settings_patch;
use crate::framework::space;
use crate::framework::space::index as space_index;
use tauri::AppHandle;

/// 列出本机空间：索引条目 + 兼容承载位默认空间（旧安装没有索引也要看得到默认空间）
#[tauri::command]
pub fn data_spaces_list(app: AppHandle) -> Result<Vec<SpaceSummary>, String> {
    let index = space_index::read_index(&app)?;
    let active = space_index::read_active_id(&app)
        .or_else(|| space::current_id().ok())
        .unwrap_or_default();
    let mut list = Vec::with_capacity(index.len());
    for (space_id, record) in &index {
        let source = record.imported_from.as_ref();
        // 默认空间在索引里有条目且未命名时用固定文案（与 space::display_name 同一口径）
        let name = if record.is_default && record.name.trim().is_empty() {
            space_index::DEFAULT_SPACE_NAME.to_string()
        } else {
            record.display_name(space_id)
        };
        list.push(SpaceSummary {
            space_id: space_id.clone(),
            name,
            created_at: record.created_at.clone(),
            active: &active == space_id,
            imported: source.is_some(),
            source_space_name: source.map(|item| item.source_space_name.clone()),
            imported_at: source.map(|item| item.imported_at.clone()),
            counts: source.map(|item| item.counts.clone()).unwrap_or_default(),
        });
    }
    // 活动空间排最前，其余按时间倒序（新建/导入的空间更容易被找到）
    list.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
    Ok(list)
}

/// 切换活动空间：只写设备级指针，重启后由启动解析生效（不热切换）
#[tauri::command]
pub async fn data_space_switch(
    app: AppHandle,
    space_id: String,
    revision: Option<u64>,
) -> Result<SpaceSwitchResult, String> {
    if !space::is_valid_space_id(&space_id) {
        return Err(format!("空间 id 非法：{space_id}"));
    }
    let index = space_index::read_index(&app)?;
    if !index.contains_key(&space_id) {
        return Err(format!("空间不存在或尚未登记：{space_id}"));
    }
    let name = space_index::display_name(&app, &space_id);
    // 维护互斥：空间切换与导入提交、根迁移互斥（前端禁用按钮不算锁）
    let _guard = maintenance_guard().await;
    let mut patch = serde_json::Map::new();
    patch.insert(
        space::KEY_ACTIVE_SPACE_ID.to_string(),
        serde_json::Value::String(space_id.clone()),
    );
    let next = settings_patch(app, revision, patch)?;
    Ok(SpaceSwitchResult {
        space_id,
        name,
        restart_required: true,
        revision: next,
    })
}

/// 列出当前空间的导入前快照（新的在前）
#[tauri::command]
pub fn data_backup_list(_app: AppHandle) -> Result<Vec<BackupSummary>, String> {
    let ctx = crate::framework::context::current().ok_or("数据上下文未初始化")?;
    let space_id = ctx.space_id().to_string();
    let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
    let snapshots = crate::framework::data_transfer::backup::list_snapshots(device_root)?;
    Ok(snapshots
        .into_iter()
        .filter(|(_, manifest)| manifest.space_id == space_id)
        .map(|(dir, manifest)| BackupSummary {
            dir: dir
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            space_id: manifest.space_id,
            created_at: manifest.created_at,
            files: manifest.files,
        })
        .collect())
}

/// 还原到导入前：把快照里的存储写回当前空间（覆盖现状），完成后广播刷新
///
/// 与导入提交同款互斥与写冻结；还原对象必须是当前空间的快照（别的空间的快照拒绝）。
#[tauri::command]
pub async fn data_backup_restore(app: AppHandle, dir: String) -> Result<(), String> {
    let ctx = crate::framework::context::current().ok_or("数据上下文未初始化")?;
    let space_id = ctx.space_id().to_string();
    let _guard = maintenance_guard().await;
    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let device_root = crate::framework::context::root().ok_or("数据上下文未初始化")?;
        let snapshot_dir =
            crate::framework::data_transfer::backup::backups_dir(device_root).join(&dir);
        // 防目录穿越：目录名不允许含路径分隔符
        if dir.contains(['/', '\\']) || dir.is_empty() {
            return Err("快照目录名非法".into());
        }
        let snapshots = crate::framework::data_transfer::backup::list_snapshots(device_root)?;
        let Some((_, manifest)) = snapshots.iter().find(|(path, _)| {
            path.file_name().map(|n| n.to_string_lossy().to_string()) == Some(dir.clone())
        }) else {
            return Err("快照不存在（可能已被清理）".into());
        };
        if manifest.space_id != space_id {
            return Err("该快照属于别的空间，拒绝还原到当前空间".into());
        }
        let _freeze = crate::framework::context::WriteFreezeGuard::begin()?;
        crate::framework::data_transfer::backup::restore_snapshot(device_root, &snapshot_dir)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("还原任务失败: {e}"))??;
    // 还原成功后广播全量刷新（涉及的数据集无法从快照精确反推，发全量）
    crate::framework::data_transfer::emit_space_data_changed(&app_clone, &["*".to_string()]);
    Ok(())
}

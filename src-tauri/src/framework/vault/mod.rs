//! Vault 凭证管理 · 门面（框架级能力，不是插件；落地 docs/standards/02-架构.md 预留的 secrets 抽象）
//! - 设计书：docs/plugins/vault/设计.md（唯一事实源）
//! - 职责：7 个框架命令薄层（参数校验 → store/export 服务层）+ ipc_registry 入库 + register
//! - 引用模型：插件 profile 只存 credentialId；后端解析走 crate 内 API `resolve()`（store.rs，
//!   不做成 Tauri 命令，明文不过 IPC）；前端仅 vault_reveal 例外路径取明文
//! - SSH/DNS 已支持可选 Vault 引用，同时保留原手工凭据路径
//! - 不做：强制搬迁/删除旧凭据、主密码解锁、stronghold、审计日志

mod export;
pub mod models;
mod store;

pub use models::{
    Credential, CredentialFields, CredentialSavePayload, CredentialSummary, VaultDeleteResult,
    VaultImportResult,
};
// crate 内解析 API：SSH/DNS 等插件在 Rust 侧解析 credentialId 后直接建连/调用云 API。
pub use store::resolve;
// crate 内只读导出面：数据导出（sync L2）要枚举凭证原文与脱敏摘要，走这两个窄入口，
// 不把整个 store 开成 pub(crate)（其余写路径仍只属于本模块）。
// 导入侧另加两个定位入口：隔离导入要把凭证按**新空间**的主密钥写进暂存目录，
// 只能走「指定目录 + 指定密钥库」这层，不能用依赖当前空间的默认路径。
pub(crate) use store::{
    read_all as credentials_read_all, read_all_at as credentials_read_all_at,
    summary_of as credential_summary, write_all_at as credentials_write_all_at,
};

use tauri::AppHandle;

use crate::framework::credential_refs;

/// 入参校验：名称非空、kind 与 fields 标签一致、各类型必填字段非空
fn validate_payload(payload: &CredentialSavePayload) -> Result<(), String> {
    if payload.name.trim().is_empty() {
        return Err("凭证名称不能为空".into());
    }
    if payload.kind != payload.fields.kind() {
        return Err("凭证类型与字段结构不一致（kind 与 fields.type 不匹配）".into());
    }
    match &payload.fields {
        CredentialFields::Password { username, password } => {
            if username.trim().is_empty() || password.is_empty() {
                return Err("用户名与密码不能为空".into());
            }
        }
        CredentialFields::SshKey {
            username,
            private_key,
            ..
        } => {
            if username.trim().is_empty() || private_key.trim().is_empty() {
                return Err("用户名与私钥不能为空".into());
            }
        }
        CredentialFields::ApiToken { token } => {
            if token.trim().is_empty() {
                return Err("token 不能为空".into());
            }
        }
        CredentialFields::AccessKeyPair {
            access_key_id,
            access_key_secret,
        } => {
            if access_key_id.trim().is_empty() || access_key_secret.is_empty() {
                return Err("AccessKey ID 与 Secret 不能为空".into());
            }
        }
        CredentialFields::Custom { entries } => {
            if entries.is_empty() {
                return Err("自定义凭证至少需要一个字段".into());
            }
            if entries.iter().any(|e| e.key.trim().is_empty()) {
                return Err("自定义字段的键名不能为空".into());
            }
        }
    }
    Ok(())
}

/// 凭证列表（脱敏摘要：id/name/kind/掩码摘要/时间，无明文）
#[tauri::command]
pub fn vault_list(app: AppHandle) -> Result<Vec<CredentialSummary>, String> {
    Ok(store::read_all(&app)?
        .iter()
        .map(store::summary_of)
        .collect())
}

/// 新增 / 更新凭证（id 可选 upsert；更新保留 created_at，刷新 updated_at）
#[tauri::command]
pub fn vault_save(
    app: AppHandle,
    payload: CredentialSavePayload,
) -> Result<CredentialSummary, String> {
    crate::framework::context::assert_writable()?;
    validate_payload(&payload)?;
    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    let mut all = store::read_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
    )?;
    let now = chrono::Utc::now().timestamp();
    let saved = match &payload.id {
        // 更新：保留 id 与 created_at
        Some(id) => {
            let existing = all
                .iter_mut()
                .find(|c| &c.id == id)
                .ok_or_else(|| format!("凭证不存在或已删除（id: {id}）"))?;
            existing.name = payload.name.trim().to_string();
            existing.kind = payload.fields.kind();
            existing.fields = payload.fields.clone();
            existing.note = payload.note.clone();
            existing.updated_at = now;
            existing.clone()
        }
        // 新增：uuid v4
        None => {
            let credential = Credential {
                id: uuid::Uuid::new_v4().to_string(),
                name: payload.name.trim().to_string(),
                kind: payload.fields.kind(),
                fields: payload.fields.clone(),
                note: payload.note.clone(),
                created_at: now,
                updated_at: now,
            };
            all.push(credential.clone());
            credential
        }
    };
    store::write_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &all,
        &crate::framework::space::current_id()?,
    )?;
    Ok(store::summary_of(&saved))
}

/// 删除凭证：删除前由后端重新核对当前引用，不依赖前端可能过期的计数
///
/// 参数语义：
/// - `expected_references`：前端确认弹窗里展示的引用总数；与当前不一致说明确认期间引用变了，
///   直接返回错误让界面重新提示（不能拿旧计数当作已经确认过）。
/// - `force`：用户显式确认删除。存在引用或有插件扫描失败（计数未知）时，没有显式确认一律拒绝。
#[tauri::command]
pub fn vault_delete(
    app: AppHandle,
    id: String,
    force: Option<bool>,
    expected_references: Option<usize>,
) -> Result<VaultDeleteResult, String> {
    crate::framework::context::assert_writable()?;
    let summary = credential_refs::summarize(&app, &id);
    let referenced_by = summary.total;
    if !force.unwrap_or(false) {
        if summary.has_unknown() {
            return Err(format!(
                "有插件暂时无法统计引用（{}），为避免误删请先确认后再强制删除",
                summary.unknown_owners.join("、")
            ));
        }
        if summary.total > 0 {
            return Err(format!(
                "该凭证仍被 {referenced_by} 处配置引用，请先改绑或确认强制删除"
            ));
        }
    } else if let Some(expected) = expected_references {
        if !summary.matches_total(expected) {
            return Err(format!(
                "引用情况已变化（确认时 {expected} 处，当前 {referenced_by} 处），请重新确认后删除"
            ));
        }
    }
    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    let mut all = store::read_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
    )?;
    let before = all.len();
    all.retain(|c| c.id != id);
    if all.len() == before {
        return Ok(VaultDeleteResult {
            ok: false,
            error: Some(format!("凭证不存在或已删除（id: {id}）")),
            referenced_by,
        });
    }
    store::write_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &all,
        &crate::framework::space::current_id()?,
    )?;
    Ok(VaultDeleteResult {
        ok: true,
        error: None,
        referenced_by,
    })
}

/// 查询凭证引用概况（按 owner 批量扫描各插件的自报能力）
///
/// 返回每个插件的引用对象清单与扫描状态；扫描失败的插件进入 `unknownOwners`，
/// 前端必须显示「计数未知」，不能当成 0 条引用。
#[tauri::command]
pub fn vault_credential_references(
    app: AppHandle,
    id: String,
) -> credential_refs::ReferenceSummary {
    credential_refs::summarize(&app, &id)
}

/// 读取单条凭证明文（仅用户点「显示/复制」时调用；列表永远走脱敏数据）
#[tauri::command]
pub fn vault_reveal(app: AppHandle, id: String) -> Result<Credential, String> {
    store::resolve(&app, &id)
}

/// 凭证保护状态（T04-5）：主密钥实际来源（系统密钥库 / 本地降级文件 / 不可用）与可用性。
/// 设置页据此持久展示「是否真的受系统密钥库保护」，无法可用时给出原因。
#[tauri::command]
pub fn vault_protection_status(
    app: AppHandle,
) -> Result<crate::framework::secure_store::ProtectionStatus, String> {
    store::protection_status(&app)
}

/// 导出 .pbvault 备份（用户设一次性密码 → Argon2id 派生密钥 → AES-256-GCM；路径由前端 dialog 选定）
#[tauri::command]
pub async fn vault_export(app: AppHandle, path: String, password: String) -> Result<(), String> {
    let all = store::read_all(&app)?;
    let plain = serde_json::to_vec(&all).map_err(|e| e.to_string())?;
    // Argon2id 是 CPU 重负载（数百 ms），异步命令里必须挪到阻塞线程池（规范 §4）
    let backup =
        tauri::async_runtime::spawn_blocking(move || export::encrypt_backup(&password, &plain))
            .await
            .map_err(|e| format!("加密任务失败: {e}"))??;
    export::write_backup_file(std::path::Path::new(&path), &backup)
}

/// 导入 .pbvault 备份（overwrite=false 合并：同 id 冲突跳过；true 全量覆盖）。
/// 现有 vault 读不出（密钥丢失 / 文件损坏）时，先把原文件改名留档再按空库导入——绝不静默清空。
#[tauri::command]
pub async fn vault_import(
    app: AppHandle,
    path: String,
    password: String,
    overwrite: bool,
) -> Result<VaultImportResult, String> {
    // 1) 备份文件由用户密码解开（不依赖主密钥，密钥丢失场景也能导入）
    let backup = export::read_backup_file(std::path::Path::new(&path))?;
    // Argon2id 解密同属 CPU 重负载，挪到阻塞线程池（规范 §4）
    let plain =
        tauri::async_runtime::spawn_blocking(move || export::decrypt_backup(&password, &backup))
            .await
            .map_err(|e| format!("解密任务失败: {e}"))??;
    let imported: Vec<Credential> =
        serde_json::from_slice(&plain).map_err(|e| format!("备份内容解析失败: {e}"))?;

    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    // 2) 现有库读不出 → 原文件改名留档（防误删），按空库继续
    let existing = match store::read_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &crate::framework::space::current_id()?,
    ) {
        Ok(all) => all,
        Err(e) => {
            eprintln!("[vault] 现有凭证库无法读取（{e}），导入前已将原文件改名留档");
            store::orphan_vault_file(&dir)?;
            Vec::new()
        }
    };
    // 3) 合并 / 覆盖
    let (merged, imported_count, skipped) = if overwrite {
        let count = imported.len();
        (imported, count, 0)
    } else {
        let mut merged = existing;
        let mut count = 0;
        let mut skipped = 0;
        for credential in imported {
            if merged.iter().any(|c| c.id == credential.id) {
                skipped += 1;
            } else {
                merged.push(credential);
                count += 1;
            }
        }
        (merged, count, skipped)
    };
    store::write_all_at(
        &dir,
        &crate::framework::space::keyring_store()?,
        &merged,
        &crate::framework::space::current_id()?,
    )?;
    Ok(VaultImportResult {
        imported: imported_count,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::models::{CredentialKind, CustomEntry};
    use super::*;

    /// 入参校验：名称空 / kind 与 fields 不一致 / 各类型必填缺失
    #[test]
    fn validate_payload_rules() {
        let ok = CredentialSavePayload {
            id: None,
            name: "生产 MySQL".into(),
            kind: CredentialKind::Password,
            fields: CredentialFields::Password {
                username: "root".into(),
                password: "s3cret".into(),
            },
            note: String::new(),
        };
        assert!(validate_payload(&ok).is_ok());

        // 空名称
        let mut bad = CredentialSavePayload {
            name: "  ".into(),
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());

        // kind 与 fields.type 不一致
        bad = CredentialSavePayload {
            kind: CredentialKind::ApiToken,
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());

        // ssh-key 缺私钥
        bad = CredentialSavePayload {
            kind: CredentialKind::SshKey,
            fields: CredentialFields::SshKey {
                username: "root".into(),
                private_key: "  ".into(),
                passphrase: None,
            },
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());

        // api-token 空 token
        bad = CredentialSavePayload {
            kind: CredentialKind::ApiToken,
            fields: CredentialFields::ApiToken { token: " ".into() },
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());

        // access-key-pair 缺 secret
        bad = CredentialSavePayload {
            kind: CredentialKind::AccessKeyPair,
            fields: CredentialFields::AccessKeyPair {
                access_key_id: "AKID".into(),
                access_key_secret: String::new(),
            },
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());

        // custom 空条目 / 空键名
        bad = CredentialSavePayload {
            kind: CredentialKind::Custom,
            fields: CredentialFields::Custom { entries: vec![] },
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());
        bad = CredentialSavePayload {
            kind: CredentialKind::Custom,
            fields: CredentialFields::Custom {
                entries: vec![CustomEntry {
                    key: " ".into(),
                    value: "v".into(),
                    secret: true,
                }],
            },
            ..ok.clone_for_test()
        };
        assert!(validate_payload(&bad).is_err());
    }

    /// Credential serde：kind kebab-case、fields 内部标签 type、camelCase 字段名
    #[test]
    fn credential_serde_shape() {
        let credential = Credential {
            id: "c1".into(),
            name: "腾讯云 CAM".into(),
            kind: CredentialKind::AccessKeyPair,
            fields: CredentialFields::AccessKeyPair {
                access_key_id: "AKIDEXAMPLE".into(),
                access_key_secret: "secret".into(),
            },
            note: "备注".into(),
            created_at: 100,
            updated_at: 200,
        };
        let json = serde_json::to_value(&credential).unwrap();
        assert_eq!(json["kind"], "access-key-pair");
        assert_eq!(json["fields"]["type"], "access-key-pair");
        assert_eq!(json["fields"]["accessKeyId"], "AKIDEXAMPLE");
        assert_eq!(json["createdAt"], 100);
        // 往返一致
        let back: Credential = serde_json::from_value(json).unwrap();
        assert_eq!(back, credential);
    }

    /// 测试辅助：从样板 payload 克隆（字段逐个覆盖）
    impl CredentialSavePayload {
        /// 克隆测试样板（CredentialSavePayload 业务上无需 Clone，仅单测用）
        fn clone_for_test(&self) -> Self {
            CredentialSavePayload {
                id: self.id.clone(),
                name: self.name.clone(),
                kind: self.kind,
                fields: self.fields.clone(),
                note: self.note.clone(),
            }
        }
    }
}

//! Vault 凭证管理 · 门面（框架级能力，不是插件；落地 docs/02-architecture.md 预留的 secrets 抽象）
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

use tauri::AppHandle;

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
    validate_payload(&payload)?;
    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    let mut all = store::read_all_at(&dir, &store::KeyringStore)?;
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
    store::write_all_at(&dir, &store::KeyringStore, &all)?;
    Ok(store::summary_of(&saved))
}

/// 删除凭证（返回被引用计数供前端提示；引用扫描随设计 §6 迁移接入）
#[tauri::command]
pub fn vault_delete(app: AppHandle, id: String) -> Result<VaultDeleteResult, String> {
    let referenced_by = store::reference_count(&app, &id);
    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    let mut all = store::read_all_at(&dir, &store::KeyringStore)?;
    let before = all.len();
    all.retain(|c| c.id != id);
    if all.len() == before {
        return Ok(VaultDeleteResult {
            ok: false,
            error: Some(format!("凭证不存在或已删除（id: {id}）")),
            referenced_by,
        });
    }
    store::write_all_at(&dir, &store::KeyringStore, &all)?;
    Ok(VaultDeleteResult {
        ok: true,
        error: None,
        referenced_by,
    })
}

/// 删除前查询后端持久化插件中的凭证引用数；浏览器侧引用由前端登记表补充。
#[tauri::command]
pub fn vault_reference_count(app: AppHandle, id: String) -> usize {
    store::reference_count(&app, &id)
}

/// 读取单条凭证明文（仅用户点「显示/复制」时调用；列表永远走脱敏数据）
#[tauri::command]
pub fn vault_reveal(app: AppHandle, id: String) -> Result<Credential, String> {
    store::resolve(&app, &id)
}

/// 导出 .pbvault 备份（用户设一次性密码 → Argon2id 派生密钥 → AES-256-GCM；路径由前端 dialog 选定）
#[tauri::command]
pub async fn vault_export(app: AppHandle, path: String, password: String) -> Result<(), String> {
    let all = store::read_all(&app)?;
    let plain = serde_json::to_vec(&all).map_err(|e| e.to_string())?;
    let backup = export::encrypt_backup(&password, &plain)?;
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
    let plain = export::decrypt_backup(&password, &backup)?;
    let imported: Vec<Credential> =
        serde_json::from_slice(&plain).map_err(|e| format!("备份内容解析失败: {e}"))?;

    let _guard = store::vault_lock().lock().map_err(|e| e.to_string())?;
    let dir = store::data_dir_of(&app)?;
    // 2) 现有库读不出 → 原文件改名留档（防误删），按空库继续
    let existing = match store::read_all_at(&dir, &store::KeyringStore) {
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
    store::write_all_at(&dir, &store::KeyringStore, &merged)?;
    Ok(VaultImportResult {
        imported: imported_count,
        skipped,
    })
}

/// 框架装配：7 个命令全量入 IPC 注册表（命令由 framework::invoke_handler 总 handler 分派）
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    super::ipc_registry::register(&[
        (
            "vault_list",
            "凭证列表（脱敏摘要：id/name/kind/掩码/时间，无明文）",
        ),
        (
            "vault_save",
            "新增/更新凭证（payload 打包，id 可选 upsert）",
        ),
        ("vault_delete", "删除凭证（返回被引用计数供前端提示）"),
        ("vault_reference_count", "删除前查询后端插件凭证引用数"),
        (
            "vault_reveal",
            "读取单条凭证明文（仅用户点显示/复制时调用）",
        ),
        (
            "vault_export",
            "密码加密导出 .pbvault 备份（Argon2id 派生密钥）",
        ),
        (
            "vault_import",
            "解密导入 .pbvault 备份（合并/覆盖由 UI 选择）",
        ),
    ])
    .expect("IPC 命令重复注册");
    builder
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

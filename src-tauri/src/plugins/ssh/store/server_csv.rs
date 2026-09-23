//! 服务器 CSV 文件边界。导出仅可读取本地密码，不访问凭证库与私钥。
use super::{local_auth, profiles, with_db, ProfileState};
use crate::plugins::ssh::models::AuthMethod;
use std::io::Read;
use tauri::{AppHandle, State};

/// 读取用户选择的 UTF-8 CSV，限制导入体积。
#[tauri::command]
pub async fn ssh_server_csv_read(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = std::fs::File::open(path).map_err(|_| "无法打开 CSV 文件")?;
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "读取 CSV 失败")?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("CSV 不可超过 2 MiB".into());
        }
        String::from_utf8(bytes).map_err(|_| "请将文件保存为 UTF-8 编码".into())
    })
    .await
    .map_err(|_| "读取任务中断".to_string())?
}

/// 写入用户选择的模板或脱敏结果报告。
#[tauri::command]
pub async fn ssh_server_csv_write(path: String, content: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::write(path, content).map_err(|_| "写入 CSV 失败".into())
    })
    .await
    .map_err(|_| "写入任务中断".to_string())?
}

fn cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// 按显式选择导出服务器，密码直接写入文件，不回传列表界面。
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_server_csv_export(
    app: AppHandle,
    state: State<'_, ProfileState>,
    path: String,
    ids: Vec<String>,
    include_password: bool,
) -> Result<usize, String> {
    let (content, count) = with_db(&app, &state, |db| export_csv(db, &ids, include_password))?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::write(path, content).map_err(|_| "导出文件写入失败".to_string())
    })
    .await
    .map_err(|_| "导出任务中断".to_string())??;
    log::info!("服务器 CSV 导出完成 count={count}");
    Ok(count)
}

/// 使用同一数据库快照生成完整 CSV，IO 留在阻塞工作线程。
fn export_csv(
    db: &rusqlite::Connection,
    ids: &[String],
    include_password: bool,
) -> Result<(String, usize), String> {
    let mut profiles = profiles::list_profiles(db)?;
    for profile in &mut profiles {
        local_auth::annotate(db, profile)?;
    }
    let groups = profiles::list_groups(db)?;
    let mut output = String::from(
        "\u{feff}name,host,port,username,password,group,remark,auth_type,password_source\r\n",
    );
    let mut count = 0;
    for profile in profiles.iter().filter(|p| ids.contains(&p.id)) {
        let password = if include_password && profile.auth_method == AuthMethod::Password {
            local_auth::export_password(db, profile)?.unwrap_or_default()
        } else {
            String::new()
        };
        let source = if profile.credential_ref.is_some() {
            "vault_omitted"
        } else if !password.is_empty() {
            "local"
        } else if profile.has_local_auth {
            "local_omitted"
        } else {
            "none"
        };
        let group = groups
            .iter()
            .find(|g| Some(&g.id) == profile.group_id.as_ref())
            .map(|g| g.name.as_str())
            .unwrap_or("");
        let values = [
            profile.name.clone(),
            profile.host.clone(),
            profile.port.to_string(),
            profile.username.clone(),
            password,
            group.into(),
            profile.remark.clone().unwrap_or_default(),
            profiles::auth_method_to_str(profile.auth_method).into(),
            source.into(),
        ];
        output.push_str(&values.iter().map(|v| cell(v)).collect::<Vec<_>>().join(","));
        output.push_str("\r\n");
        count += 1;
    }
    if count != ids.iter().collect::<std::collections::HashSet<_>>().len() {
        return Err("服务器列表已变化，请重新选择导出范围".into());
    }
    Ok((output, count))
}

#[cfg(test)]
mod tests {
    #[test]
    fn quotes_and_line_breaks_roundtrip_as_csv_cells() {
        assert_eq!(super::cell(" a,\"b\"\n"), "\" a,\"\"b\"\"\n\"");
    }
    #[test]
    fn export_only_selected_local_passwords_and_never_vault_or_key() {
        use crate::plugins::ssh::models::{CredentialOverride, ServerProfile};
        let db = super::super::open_memory();
        let mut p = ServerProfile {
            id: "local".into(),
            name: "local".into(),
            host: "host".into(),
            port: 22,
            username: "root".into(),
            auth_method: super::AuthMethod::Password,
            credential_ref: None,
            has_local_auth: false,
            group_id: None,
            remark: None,
            last_connected_at: None,
        };
        super::profiles::upsert_profile(&db, &p, 1).unwrap();
        super::local_auth::save(
            &db,
            &p,
            &CredentialOverride {
                password: Some("test-password".into()),
                private_key: None,
                passphrase: None,
            },
        )
        .unwrap();
        let plain = super::export_csv(&db, &["local".into()], false).unwrap().0;
        assert!(!plain.contains("test-password"));
        assert!(plain.contains("local_omitted"));
        assert!(super::export_csv(&db, &["local".into()], true)
            .unwrap()
            .0
            .contains("test-password"));
        p.id = "vault".into();
        p.credential_ref = Some("reference".into());
        super::profiles::upsert_profile(&db, &p, 1).unwrap();
        let vault = super::export_csv(&db, &["vault".into()], true).unwrap().0;
        assert!(vault.contains("vault_omitted"));
        assert!(!vault.contains("test-password"));
        p.id = "key".into();
        p.credential_ref = None;
        p.auth_method = super::AuthMethod::PrivateKey;
        super::profiles::upsert_profile(&db, &p, 1).unwrap();
        super::local_auth::save(
            &db,
            &p,
            &CredentialOverride {
                password: None,
                private_key: Some("test-private-key".into()),
                passphrase: None,
            },
        )
        .unwrap();
        assert!(!super::export_csv(&db, &["key".into()], true)
            .unwrap()
            .0
            .contains("test-private-key"));
        assert!(super::export_csv(&db, &["missing".into()], false).is_err());
    }
}

//! SSH 本地明文认证。仅连接时读取，不回传秘密，不创建 Vault 条目、不使用系统密钥。

use super::profiles::auth_method_to_str;
use crate::plugins::ssh::models::{AuthMethod, CredentialOverride, ServerProfile};
use rusqlite::{params, Connection, OptionalExtension};

/// 只读取与当前服务器连接参数完全匹配的认证，防改主机后误发旧密码。
pub(crate) fn get(
    conn: &Connection,
    profile: &ServerProfile,
) -> Result<Option<CredentialOverride>, String> {
    if profile.credential_ref.is_some() {
        return Ok(None);
    }
    conn.query_row(
        "SELECT password, private_key, passphrase FROM ssh_local_auth
         WHERE profile_id=?1 AND host=?2 AND port=?3 AND username=?4 AND auth_method=?5",
        params![
            profile.id,
            profile.host,
            profile.port,
            profile.username,
            auth_method_to_str(profile.auth_method)
        ],
        |row| {
            Ok(CredentialOverride {
                password: row.get(0)?,
                private_key: row.get(1)?,
                passphrase: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("读取本地认证失败: {e}"))
}

/// 与档案在同一事务中保存；空输入只允许保留已存在且参数匹配的认证。
pub(crate) fn save(
    conn: &Connection,
    profile: &ServerProfile,
    value: &CredentialOverride,
) -> Result<(), String> {
    let has_input = value.password.as_deref().is_some_and(|s| !s.is_empty())
        || value.private_key.as_deref().is_some_and(|s| !s.is_empty())
        || value.passphrase.as_deref().is_some_and(|s| !s.is_empty());
    if !has_input {
        return if get(conn, profile)?.is_some() {
            Ok(())
        } else {
            Err("请填写要本地保存的密码或私钥".into())
        };
    }
    let complete = match profile.auth_method {
        AuthMethod::Password => value.password.as_deref().is_some_and(|s| !s.is_empty()),
        AuthMethod::PrivateKey => value
            .private_key
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        AuthMethod::PrivateKeyWithPassphrase => {
            value
                .private_key
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
                && value.passphrase.as_deref().is_some_and(|s| !s.is_empty())
        }
    };
    if !complete {
        return Err("本地保存需要完整的密码或私钥认证信息".into());
    }
    let password_mode = profile.auth_method == AuthMethod::Password;
    conn.execute(
        "INSERT OR REPLACE INTO ssh_local_auth
         (profile_id, host, port, username, auth_method, password, private_key, passphrase)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            profile.id,
            profile.host,
            profile.port,
            profile.username,
            auth_method_to_str(profile.auth_method),
            if password_mode {
                value.password.as_deref()
            } else {
                None
            },
            if password_mode {
                None
            } else {
                value.private_key.as_deref()
            },
            if profile.auth_method == AuthMethod::PrivateKeyWithPassphrase {
                value.passphrase.as_deref()
            } else {
                None
            }
        ],
    )
    .map_err(|e| format!("保存本地认证失败: {e}"))?;
    Ok(())
}

/// 普通配置序列化不携带秘密；列表只附带是否本地保存的状态。
pub(crate) fn annotate(conn: &Connection, profile: &mut ServerProfile) -> Result<(), String> {
    profile.has_local_auth = get(conn, profile)?.is_some();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::ssh::store::{open_memory, profiles::*};

    fn profile() -> ServerProfile {
        serde_json::from_value(serde_json::json!({
            "id": "local", "name": "测试", "host": "example.test", "port": 22,
            "username": "root", "authMethod": "password", "hasLocalAuth": true
        }))
        .unwrap()
    }

    #[test]
    fn local_password_update_preserve_export_and_delete() {
        let conn = open_memory();
        let mut p = profile();
        assert!(!p.has_local_auth);
        upsert_profile(&conn, &p, 1).unwrap();
        for password in [" old fixture ", " corrected fixture "] {
            save(
                &conn,
                &p,
                &CredentialOverride {
                    password: Some(password.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            save(&conn, &p, &CredentialOverride::default()).unwrap();
            assert_eq!(
                get(&conn, &p).unwrap().unwrap().password.as_deref(),
                Some(password)
            );
        }
        annotate(&conn, &mut p).unwrap();
        assert!(p.has_local_auth);
        let exported = serde_json::to_value(list_profiles(&conn).unwrap()).unwrap();
        assert!(exported[0].get("password").is_none());
        assert!(exported[0].get("hasLocalAuth").is_none());
        delete_profile(&conn, &p.id).unwrap();
        assert!(get(&conn, &p).unwrap().is_none());
    }

    #[test]
    fn changed_target_requires_new_secret_and_failed_save_rolls_back() {
        let conn = open_memory();
        let p = profile();
        upsert_profile(&conn, &p, 1).unwrap();
        save(
            &conn,
            &p,
            &CredentialOverride {
                password: Some("fixture".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let mut changed = p.clone();
        changed.host = "other.test".into();
        {
            let tx = conn.unchecked_transaction().unwrap();
            upsert_profile(&tx, &changed, 2).unwrap();
            assert!(save(&tx, &changed, &CredentialOverride::default()).is_err());
        }
        assert_eq!(get_profile(&conn, &p.id).unwrap().host, p.host);
        assert!(get(&conn, &p).unwrap().is_some());
        upsert_profile(&conn, &changed, 3).unwrap();
        assert!(get(&conn, &p).unwrap().is_none());
        assert!(get(&conn, &changed).unwrap().is_none());
    }

    #[test]
    fn vault_switch_and_clear_remove_local_auth() {
        let conn = open_memory();
        let mut p = profile();
        let value = CredentialOverride {
            password: Some("fixture".into()),
            ..Default::default()
        };
        upsert_profile(&conn, &p, 1).unwrap();
        save(&conn, &p, &value).unwrap();
        p.credential_ref = Some("vault-fixture".into());
        upsert_profile(&conn, &p, 2).unwrap();
        p.credential_ref = None;
        assert!(get(&conn, &p).unwrap().is_none());
        save(&conn, &p, &value).unwrap();
        clear_profiles(&conn).unwrap();
        assert!(get(&conn, &p).unwrap().is_none());
    }
}

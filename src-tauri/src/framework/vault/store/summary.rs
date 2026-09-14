//! Vault 展示用脱敏摘要：列表只出「非秘密字段 + 掩码」，秘密明文不离开读取入口

use crate::framework::vault::models::{Credential, CredentialFields, CredentialSummary};

/// 秘密值掩码：≤6 字符全掩码；否则前 3 + **** + 后 3（如 AKI****xyz）
pub(super) fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 6 {
        return "••••".into();
    }
    let head: String = chars.iter().take(3).collect();
    let tail: String = chars.iter().skip(chars.len() - 3).collect();
    format!("{head}****{tail}")
}

/// 生成列表用脱敏摘要（无明文秘密；用户名等非秘密字段可直接展示）
pub(crate) fn summary_of(credential: &Credential) -> CredentialSummary {
    let masked = match &credential.fields {
        // 用户名不是秘密，直接展示；空用户名退回掩码
        CredentialFields::Password { username, password } => {
            if username.is_empty() {
                mask_secret(password)
            } else {
                username.clone()
            }
        }
        CredentialFields::SshKey { username, .. } => {
            if username.is_empty() {
                "私钥凭证".to_string()
            } else {
                username.clone()
            }
        }
        CredentialFields::ApiToken { token } => mask_secret(token),
        CredentialFields::AccessKeyPair { access_key_id, .. } => mask_secret(access_key_id),
        CredentialFields::Custom { entries } => {
            format!("{} 个字段", entries.len())
        }
    };
    CredentialSummary {
        id: credential.id.clone(),
        name: credential.name.clone(),
        kind: credential.kind,
        masked,
        note: credential.note.clone(),
        created_at: credential.created_at,
        updated_at: credential.updated_at,
    }
}

//! DNS 模块 · tests

#[cfg(test)]
mod cases {

    use super::super::config::apply_vault_api_token;
    use super::super::config::apply_vault_credential;
    use crate::framework::vault::Credential;
    use crate::framework::vault::CredentialFields;

    use crate::plugins::dns::models::{CloudflareConfig, ProviderConfig};

    fn credential(fields: CredentialFields) -> Credential {
        Credential {
            id: "vault-dns".into(),
            name: "云解析密钥".into(),
            kind: fields.kind(),
            fields,
            note: String::new(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn vault_access_key_overrides_manual_values() {
        let config = ProviderConfig {
            id: "manual-id".into(),
            key: "manual-key".into(),
            credential_ref: Some("vault-dns".into()),
        };
        let resolved = apply_vault_credential(
            config,
            credential(CredentialFields::AccessKeyPair {
                access_key_id: "vault-id".into(),
                access_key_secret: "vault-key".into(),
            }),
            "阿里云",
        )
        .unwrap();
        assert_eq!(resolved.id, "vault-id");
        assert_eq!(resolved.key, "vault-key");
        assert_eq!(resolved.credential_ref.as_deref(), Some("vault-dns"));
    }

    #[test]
    fn vault_dns_credential_must_be_access_key_pair() {
        let config = ProviderConfig {
            id: String::new(),
            key: String::new(),
            credential_ref: Some("vault-dns".into()),
        };
        let result = apply_vault_credential(
            config,
            credential(CredentialFields::ApiToken {
                token: "token".into(),
            }),
            "腾讯云 DNSPod",
        );
        assert!(result.is_err());
    }

    #[test]
    fn vault_api_token_overrides_manual_cloudflare_token() {
        let config = CloudflareConfig {
            token: "manual-token".into(),
            credential_ref: Some("vault-dns".into()),
        };
        let resolved = apply_vault_api_token(
            config,
            credential(CredentialFields::ApiToken {
                token: "vault-token".into(),
            }),
        )
        .unwrap();
        assert_eq!(resolved.token, "vault-token");
        assert_eq!(resolved.credential_ref.as_deref(), Some("vault-dns"));
    }

    #[test]
    fn vault_cloudflare_credential_must_be_api_token() {
        let result = apply_vault_api_token(
            CloudflareConfig::default(),
            credential(CredentialFields::AccessKeyPair {
                access_key_id: "id".into(),
                access_key_secret: "secret".into(),
            }),
        );
        assert!(result.is_err());
    }
}

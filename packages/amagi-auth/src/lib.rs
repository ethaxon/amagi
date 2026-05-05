mod audit;
mod error;
mod facade;
mod principal;
mod repository;

pub use audit::{
    AuditPreviewEnvelope, AuditWriteOutcome, AuthAuditEventType, AuthAuditWriter,
    SanitizedQueryEnvelope,
};
pub use error::{AuthError, AuthResult};
pub use facade::{
    AuthFacadeService, AuthenticatedOidcUserInfoResponse, BearerBoundPrincipalView,
    BearerPrincipalResolution, FrontendCallbackShellResponse, FrontendOidcConfigProjection,
    PrincipalResolutionResult, PrincipalView,
};
pub use principal::{AmagiPrincipal, ExternalOidcIdentity, OidcIdentityClaim, VaultAccessBoundary};
pub use repository::{AccountBindingRecord, AccountBindingRepository, AccountBindingResolution};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use amagi_config::ApiServerConfig;
    use amagi_db::DatabaseService;
    use amagi_securitydept::{
        AuthRuntime, BackendOidcModeMetadataRedemptionRequest, BackendOidcModeUserInfoRequest,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn external_identity_defaults_to_sub_identity_claim() {
        let identity = ExternalOidcIdentity::new(
            "primary",
            OidcIdentityClaim::default(),
            json!({
                "sub": "oidc-user-123",
                "email": "user@example.com"
            }),
        )
        .expect("identity builds");

        assert_eq!(identity.oidc_identity_key(), "oidc-user-123");
        assert_eq!(identity.oidc_subject(), "oidc-user-123");
        assert_eq!(identity.source_key(), "primary");
    }

    #[test]
    fn principal_resolution_does_not_grant_vault_access() {
        let principal = AmagiPrincipal::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            ExternalOidcIdentity::new(
                "workforce",
                OidcIdentityClaim::Email,
                json!({
                    "sub": "oidc-user-123",
                    "email": "user@example.com"
                }),
            )
            .expect("identity builds"),
        );

        assert_eq!(principal.vault_access(), VaultAccessBoundary::NotGranted);
        assert_eq!(
            principal.external_identity().oidc_identity_key(),
            "user@example.com"
        );
        assert_eq!(
            principal.external_identity().oidc_subject(),
            "oidc-user-123"
        );
    }

    fn sample_config(database_url: Option<String>) -> ApiServerConfig {
        let mut config = ApiServerConfig {
            default_oidc_source: Some("primary".to_owned()),
            ..ApiServerConfig::default()
        };

        config.oidc_sources.insert(
            "primary".to_owned(),
            serde_json::from_value(json!({
                "oidc": {
                    "issuer_url": "https://issuer.primary",
                    "well_known_url": "https://issuer.primary/.well-known/openid-configuration",
                    "client_id": "interactive-client",
                    "client_secret": "backend-secret"
                },
                "access_token_substrate": {
                    "audiences": ["api://amagi"]
                }
            }))
            .expect("oidc source config parses"),
        );

        if let Some(database_url) = database_url {
            config.database = serde_json::from_value(json!({
                "url": database_url,
                "auto_migrate": true,
            }))
            .expect("database config parses");
        }

        config
    }

    #[test]
    fn callback_failure_payload_filters_sensitive_query_fields() {
        let writer = AuthAuditWriter::new();
        let preview = writer.preview_callback_failed(
            "primary",
            &BTreeMap::from([
                ("code".to_owned(), "secret-code".to_owned()),
                ("state".to_owned(), "opaque-state".to_owned()),
                ("refresh_token".to_owned(), "must-not-appear".to_owned()),
                ("error".to_owned(), "access_denied".to_owned()),
            ]),
            "backend_callback_body",
            &AuthError::MissingAccessToken,
        );

        assert_eq!(preview.outcome, AuditWriteOutcome::SkippedNoOwnerContext);

        let payload_text = preview.payload.to_string();
        assert!(!payload_text.contains("secret-code"));
        assert!(!payload_text.contains("opaque-state"));
        assert!(!payload_text.contains("must-not-appear"));
        assert!(payload_text.contains("access_denied"));
    }

    #[tokio::test]
    async fn missing_access_token_emits_user_info_failed_preview() {
        let config = sample_config(None);
        let audit = AuthAuditWriter::capturing_previews();
        let service = AuthFacadeService::with_audit_writer(
            std::sync::Arc::new(config.clone()),
            AuthRuntime::from_api_config(&config),
            DatabaseService::default(),
            audit.clone(),
        );

        let error = service
            .oidc_user_info(
                "primary",
                &BackendOidcModeUserInfoRequest {
                    id_token: "header.payload.signature".to_owned(),
                },
                None,
            )
            .await
            .expect_err("missing access token should fail before runtime call");

        assert!(matches!(error, AuthError::MissingAccessToken));

        let previews = audit
            .recorded_previews()
            .expect("capturing audit writer stores previews");
        let preview = previews
            .last()
            .expect("missing access token should emit one preview");

        assert_eq!(preview.outcome, AuditWriteOutcome::SkippedNoOwnerContext);
        assert_eq!(preview.payload["event_type"], "auth.oidc.user_info.failed");
        assert_eq!(preview.payload["source_key"], "primary");
        assert_eq!(preview.payload["payload"]["surface"], "user_info");
        assert_eq!(
            preview.payload["payload"]["error_code"],
            AuthError::MissingAccessToken.code()
        );
        assert_eq!(preview.payload["payload"]["http_status"], 401);
        let payload_text = preview.payload.to_string();
        assert!(!payload_text.contains("header.payload.signature"));
        assert!(!payload_text.contains("Bearer "));
    }

    #[tokio::test]
    async fn metadata_redeem_runtime_error_emits_failed_preview() {
        let mut config = sample_config(None);
        config.external_base_url = "not-a-valid-url".to_owned();
        let audit = AuthAuditWriter::capturing_previews();
        let service = AuthFacadeService::with_audit_writer(
            std::sync::Arc::new(config.clone()),
            AuthRuntime::from_api_config(&config),
            DatabaseService::default(),
            audit.clone(),
        );

        let error = service
            .oidc_metadata_redeem(
                "primary",
                &serde_json::from_value::<BackendOidcModeMetadataRedemptionRequest>(json!({
                    "metadata_redemption_id": "missing-redemption"
                }))
                .expect("metadata redemption request parses"),
            )
            .await
            .expect_err("resolver/runtime error should surface as auth error");

        assert!(matches!(error, AuthError::SecurityDept { .. }));

        let previews = audit
            .recorded_previews()
            .expect("capturing audit writer stores previews");
        let preview = previews
            .last()
            .expect("runtime error should emit metadata redeem failed preview");

        assert_eq!(preview.outcome, AuditWriteOutcome::SkippedNoOwnerContext);
        assert_eq!(
            preview.payload["event_type"],
            "auth.oidc.metadata_redeem.failed"
        );
        assert_eq!(preview.payload["source_key"], "primary");
        assert_eq!(preview.payload["payload"]["surface"], "metadata_redeem");
        assert_eq!(preview.payload["payload"]["error_code"], error.code());
        assert_eq!(
            preview.payload["payload"]["http_status"],
            error.http_status_code()
        );
        let payload_text = preview.payload.to_string();
        assert!(!payload_text.contains("missing-redemption"));
        assert!(!payload_text.contains("secret"));
    }

    #[tokio::test]
    async fn bearer_resolution_returns_none_without_authorization_header() {
        let config = sample_config(None);
        let service = AuthFacadeService::new(
            std::sync::Arc::new(config.clone()),
            AuthRuntime::from_api_config(&config),
            DatabaseService::default(),
        );

        let resolution = service
            .authenticate_bearer_principal("primary", None)
            .await
            .expect("missing bearer header should be handled cleanly");

        assert!(resolution.is_none());
    }
}

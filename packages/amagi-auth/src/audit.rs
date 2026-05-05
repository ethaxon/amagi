use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use amagi_db::entities::audit_events;
use amagi_securitydept::VerifiedOidcUserInfo;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{AuthError, AuthResult},
    principal::AmagiPrincipal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditWriteOutcome {
    Persisted,
    SkippedNoOwnerContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAuditEventType {
    OidcStart,
    OidcCallbackSucceeded,
    OidcCallbackFailed,
    OidcRefreshSucceeded,
    OidcRefreshFailed,
    OidcUserInfoSucceeded,
    OidcUserInfoFailed,
    OidcMetadataRedeemSucceeded,
    OidcMetadataRedeemFailed,
    OidcAccountBindingCreated,
    OidcAccountBindingReused,
    PrincipalResolved,
}

impl AuthAuditEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OidcStart => "auth.oidc.start",
            Self::OidcCallbackSucceeded => "auth.oidc.callback.succeeded",
            Self::OidcCallbackFailed => "auth.oidc.callback.failed",
            Self::OidcRefreshSucceeded => "auth.oidc.refresh.succeeded",
            Self::OidcRefreshFailed => "auth.oidc.refresh.failed",
            Self::OidcUserInfoSucceeded => "auth.oidc.user_info.succeeded",
            Self::OidcUserInfoFailed => "auth.oidc.user_info.failed",
            Self::OidcMetadataRedeemSucceeded => "auth.oidc.metadata_redeem.succeeded",
            Self::OidcMetadataRedeemFailed => "auth.oidc.metadata_redeem.failed",
            Self::OidcAccountBindingCreated => "auth.oidc.account_binding.created",
            Self::OidcAccountBindingReused => "auth.oidc.account_binding.reused",
            Self::PrincipalResolved => "auth.principal.resolved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SanitizedQueryEnvelope {
    pub visible_query_keys: Vec<String>,
    pub code_present: bool,
    pub state_present: bool,
    pub error: Option<String>,
    pub error_description_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditPreviewEnvelope {
    pub outcome: AuditWriteOutcome,
    pub payload: Value,
}

#[derive(Debug, Clone, Default)]
pub struct AuthAuditWriter {
    preview_sink: Option<Arc<Mutex<Vec<AuditPreviewEnvelope>>>>,
}

impl AuthAuditWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn capturing_previews() -> Self {
        Self {
            preview_sink: Some(Arc::new(Mutex::new(Vec::new()))),
        }
    }

    pub fn recorded_previews(&self) -> Option<Vec<AuditPreviewEnvelope>> {
        self.preview_sink.as_ref().map(|sink| {
            sink.lock()
                .expect("preview sink lock is not poisoned")
                .clone()
        })
    }

    pub fn preview_start(
        &self,
        source_key: &str,
        has_post_auth_redirect_uri: bool,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcStart,
            source_key,
            json!({
                "surface": "start",
                "has_post_auth_redirect_uri": has_post_auth_redirect_uri,
            }),
        ))
    }

    pub fn preview_callback_succeeded(
        &self,
        source_key: &str,
        query: &BTreeMap<String, String>,
        surface: &'static str,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcCallbackSucceeded,
            source_key,
            json!({
                "surface": surface,
                "query": sanitize_query_envelope(query),
            }),
        ))
    }

    pub fn preview_callback_failed(
        &self,
        source_key: &str,
        query: &BTreeMap<String, String>,
        surface: &'static str,
        error: &AuthError,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcCallbackFailed,
            source_key,
            json!({
                "surface": surface,
                "query": sanitize_query_envelope(query),
                "error_code": error.code(),
                "http_status": error.http_status_code(),
            }),
        ))
    }

    pub fn preview_refresh_succeeded(&self, source_key: &str) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcRefreshSucceeded,
            source_key,
            json!({ "surface": "refresh" }),
        ))
    }

    pub fn preview_refresh_failed(
        &self,
        source_key: &str,
        error: &AuthError,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcRefreshFailed,
            source_key,
            json!({
                "surface": "refresh",
                "error_code": error.code(),
                "http_status": error.http_status_code(),
            }),
        ))
    }

    pub fn preview_metadata_redeem_succeeded(&self, source_key: &str) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcMetadataRedeemSucceeded,
            source_key,
            json!({ "surface": "metadata_redeem" }),
        ))
    }

    pub fn preview_metadata_redeem_failed(
        &self,
        source_key: &str,
        error: &AuthError,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcMetadataRedeemFailed,
            source_key,
            json!({
                "surface": "metadata_redeem",
                "error_code": error.code(),
                "http_status": error.http_status_code(),
            }),
        ))
    }

    pub fn preview_user_info_succeeded(
        &self,
        source_key: &str,
        user_info: &VerifiedOidcUserInfo,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcUserInfoSucceeded,
            source_key,
            json!({
                "surface": "user_info",
                "claim_keys": sanitize_claim_keys(user_info.claims.as_ref()),
            }),
        ))
    }

    pub fn preview_user_info_failed(
        &self,
        source_key: &str,
        error: &AuthError,
    ) -> AuditPreviewEnvelope {
        self.record_preview(self.protocol_payload(
            AuthAuditEventType::OidcUserInfoFailed,
            source_key,
            json!({
                "surface": "user_info",
                "error_code": error.code(),
                "http_status": error.http_status_code(),
            }),
        ))
    }

    pub async fn record_account_binding_created(
        &self,
        txn: &DatabaseTransaction,
        principal: &AmagiPrincipal,
    ) -> AuthResult<AuditWriteOutcome> {
        self.record_owned_event(
            txn,
            principal,
            AuthAuditEventType::OidcAccountBindingCreated,
            self.principal_payload(principal),
        )
        .await
    }

    pub async fn record_account_binding_reused(
        &self,
        txn: &DatabaseTransaction,
        principal: &AmagiPrincipal,
    ) -> AuthResult<AuditWriteOutcome> {
        self.record_owned_event(
            txn,
            principal,
            AuthAuditEventType::OidcAccountBindingReused,
            self.principal_payload(principal),
        )
        .await
    }

    pub async fn record_principal_resolved(
        &self,
        txn: &DatabaseTransaction,
        principal: &AmagiPrincipal,
    ) -> AuthResult<AuditWriteOutcome> {
        self.record_owned_event(
            txn,
            principal,
            AuthAuditEventType::PrincipalResolved,
            self.principal_payload(principal),
        )
        .await
    }

    pub fn protocol_payload(
        &self,
        event_type: AuthAuditEventType,
        source_key: &str,
        payload: Value,
    ) -> Value {
        json!({
            "event_type": event_type.as_str(),
            "source_key": source_key,
            "payload": payload,
        })
    }

    fn record_preview(&self, payload: Value) -> AuditPreviewEnvelope {
        let envelope = AuditPreviewEnvelope {
            outcome: AuditWriteOutcome::SkippedNoOwnerContext,
            payload,
        };

        if let Some(sink) = &self.preview_sink {
            sink.lock()
                .expect("preview sink lock is not poisoned")
                .push(envelope.clone());
        }

        envelope
    }

    fn principal_payload(&self, principal: &AmagiPrincipal) -> Value {
        json!({
            "source_key": principal.external_identity().source_key(),
            "oidc_identity_claim": principal.external_identity().identity_claim().as_str(),
            "claim_keys": principal.external_identity().audit_safe_claim_keys(),
            "auth_user_id": principal.auth_user_id(),
            "vault_access": "not_granted",
        })
    }

    async fn record_owned_event(
        &self,
        txn: &DatabaseTransaction,
        principal: &AmagiPrincipal,
        event_type: AuthAuditEventType,
        payload: Value,
    ) -> AuthResult<AuditWriteOutcome> {
        audit_events::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(Some(principal.user_id())),
            event_type: Set(event_type.as_str().to_owned()),
            payload_json: Set(payload),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "insert auth audit event",
        })?;

        Ok(AuditWriteOutcome::Persisted)
    }
}

fn sanitize_claim_keys(claims: Option<&serde_json::Map<String, Value>>) -> Vec<String> {
    let mut keys = claims
        .into_iter()
        .flat_map(|claims| claims.keys())
        .filter(|key| {
            !matches!(
                key.as_str(),
                "access_token"
                    | "refresh_token"
                    | "id_token"
                    | "authorization"
                    | "client_secret"
                    | "password"
                    | "secret"
                    | "code"
                    | "state"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

pub fn sanitize_query_envelope(query: &BTreeMap<String, String>) -> SanitizedQueryEnvelope {
    let visible_query_keys = query
        .keys()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "code" | "state" | "access_token" | "refresh_token" | "id_token" | "client_secret"
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    SanitizedQueryEnvelope {
        visible_query_keys,
        code_present: query.contains_key("code"),
        state_present: query.contains_key("state"),
        error: query.get("error").cloned(),
        error_description_present: query.contains_key("error_description"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_audit_event_names_match_iter5_protocol_contract() {
        let names = [
            AuthAuditEventType::OidcStart,
            AuthAuditEventType::OidcCallbackSucceeded,
            AuthAuditEventType::OidcCallbackFailed,
            AuthAuditEventType::OidcRefreshSucceeded,
            AuthAuditEventType::OidcRefreshFailed,
            AuthAuditEventType::OidcUserInfoSucceeded,
            AuthAuditEventType::OidcUserInfoFailed,
            AuthAuditEventType::OidcMetadataRedeemSucceeded,
            AuthAuditEventType::OidcMetadataRedeemFailed,
            AuthAuditEventType::OidcAccountBindingCreated,
            AuthAuditEventType::OidcAccountBindingReused,
            AuthAuditEventType::PrincipalResolved,
        ]
        .into_iter()
        .map(AuthAuditEventType::as_str)
        .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "auth.oidc.start",
                "auth.oidc.callback.succeeded",
                "auth.oidc.callback.failed",
                "auth.oidc.refresh.succeeded",
                "auth.oidc.refresh.failed",
                "auth.oidc.user_info.succeeded",
                "auth.oidc.user_info.failed",
                "auth.oidc.metadata_redeem.succeeded",
                "auth.oidc.metadata_redeem.failed",
                "auth.oidc.account_binding.created",
                "auth.oidc.account_binding.reused",
                "auth.principal.resolved",
            ]
        );
    }

    #[test]
    fn sanitize_claim_keys_filters_sensitive_material() {
        let keys = sanitize_claim_keys(Some(&serde_json::Map::from_iter([
            ("email".to_owned(), json!("user@example.org")),
            ("name".to_owned(), json!("User")),
            ("access_token".to_owned(), json!("must-not-appear")),
            ("refresh_token".to_owned(), json!("must-not-appear")),
            ("id_token".to_owned(), json!("must-not-appear")),
            ("client_secret".to_owned(), json!("must-not-appear")),
            ("password".to_owned(), json!("must-not-appear")),
            ("state".to_owned(), json!("must-not-appear")),
            ("code".to_owned(), json!("must-not-appear")),
        ])));

        assert_eq!(keys, vec!["email", "name"]);
    }
}

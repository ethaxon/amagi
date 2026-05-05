use std::{collections::BTreeMap, sync::Arc};

use securitydept_core::{
    oidc::{MokaPendingOauthStore, MokaPendingOauthStoreConfig, OidcClient},
    token_set_context::{
        access_token_substrate::{
            AccessTokenSubstrateConfig, AccessTokenSubstrateRuntime, OAuthResourceServerVerifier,
            ResolvedAccessTokenSubstrateConfig, ResourceTokenPrincipal,
        },
        backend_oidc_mode::{
            BackendOidcModeConfig, BackendOidcModeRuntime,
            MokaPendingAuthStateMetadataRedemptionConfig,
            MokaPendingAuthStateMetadataRedemptionStore, ResolvedBackendOidcModeConfig,
        },
        frontend_oidc_mode::{FrontendOidcModeConfig, ResolvedFrontendOidcModeConfig},
        orchestration::OidcSharedConfig,
    },
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::OnceCell;
use url::Url;

pub type SecurityDeptOidcSharedConfig = OidcSharedConfig;
pub type SecurityDeptBackendOidcConfig = BackendOidcModeConfig<
    MokaPendingOauthStoreConfig,
    MokaPendingAuthStateMetadataRedemptionConfig,
>;
pub type SecurityDeptFrontendOidcConfig = FrontendOidcModeConfig;
pub type SecurityDeptAccessTokenSubstrateConfig = AccessTokenSubstrateConfig;

pub(crate) type SecurityDeptResolvedBackendOidcConfig = ResolvedBackendOidcModeConfig<
    MokaPendingOauthStoreConfig,
    MokaPendingAuthStateMetadataRedemptionConfig,
>;
pub(crate) type SecurityDeptResolvedFrontendOidcConfig = ResolvedFrontendOidcModeConfig;
pub(crate) type SecurityDeptResolvedAccessTokenSubstrateConfig = ResolvedAccessTokenSubstrateConfig;

#[derive(Debug, Clone)]
pub struct AuthRuntime {
    pub securitydept: SecurityDeptAuthRuntime,
}

#[derive(Debug, Clone)]
pub struct SecurityDeptAuthRuntime {
    pub token_set: TokenSetAuthConfig,
}

#[derive(Debug, Clone)]
pub struct TokenSetAuthConfig {
    pub default_oidc_source: Option<String>,
    pub oidc_sources: BTreeMap<String, SecurityDeptOidcSourceRuntime>,
    pub facade_paths: BackendOidcFacadePaths,
    pub securitydept_backend_oidc_surface: &'static str,
    pub securitydept_frontend_oidc_surface: &'static str,
    pub securitydept_access_token_substrate_surface: &'static str,
    pub storage_policy: TokenSetStoragePolicyRuntime,
    pub extension_session_binding_required: bool,
    pub cookie_session_dashboard_only: bool,
}

#[derive(Debug, Clone)]
pub struct SecurityDeptOidcSourceRuntime {
    pub host: SecurityDeptOidcSourceHostConfig,
    pub(crate) resolved: Option<Arc<ResolvedSecurityDeptSourceConfig>>,
    pub(crate) source_resolution_error: Option<crate::SecurityDeptError>,
    pub(crate) runtime_bundle:
        Arc<OnceCell<Result<Arc<SecurityDeptSourceRuntimeBundle>, crate::SecurityDeptError>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityDeptOidcSourceHostConfig {
    pub source_key: String,
    pub backend_callback_path: String,
    pub frontend_callback_path: String,
    pub frontend_config_projection_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSetStoragePolicyRuntime {
    pub pending_state_store: String,
    pub refresh_token_material: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendOidcFacadePaths {
    pub start: String,
    pub callback: String,
}

#[derive(Debug, Clone)]
pub struct SecurityDeptHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedOidcUserInfo {
    pub subject: String,
    pub display_name: String,
    pub picture: Option<String>,
    pub issuer: Option<String>,
    pub claims: Option<serde_json::Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedBearerPrincipalFacts {
    pub source_key: String,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub audiences: Vec<String>,
    pub scopes: Vec<String>,
    pub authorized_party: Option<String>,
    pub claims: serde_json::Map<String, Value>,
}

impl VerifiedOidcUserInfo {
    pub fn claims_snapshot(&self) -> Value {
        let mut claims = self.claims.clone().unwrap_or_default();

        claims
            .entry("sub".to_owned())
            .or_insert_with(|| Value::String(self.subject.clone()));
        claims
            .entry("name".to_owned())
            .or_insert_with(|| Value::String(self.display_name.clone()));
        if let Some(issuer) = &self.issuer {
            claims
                .entry("iss".to_owned())
                .or_insert_with(|| Value::String(issuer.clone()));
        }

        Value::Object(claims)
    }
}

impl From<ResourceTokenPrincipal> for VerifiedBearerPrincipalFacts {
    fn from(value: ResourceTokenPrincipal) -> Self {
        Self {
            source_key: String::new(),
            subject: value.subject,
            issuer: value.issuer,
            audiences: value.audiences,
            scopes: value.scopes,
            authorized_party: value.authorized_party,
            claims: value.claims.into_iter().collect(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResolvedSecurityDeptSourceConfig {
    pub(crate) _shared_oidc: SecurityDeptOidcSharedConfig,
    pub(crate) backend_oidc: SecurityDeptResolvedBackendOidcConfig,
    pub(crate) frontend_oidc: SecurityDeptResolvedFrontendOidcConfig,
    pub(crate) access_token_substrate: SecurityDeptResolvedAccessTokenSubstrateConfig,
    pub(crate) external_base_url: String,
}

pub(crate) struct SecurityDeptSourceRuntimeBundle {
    pub(crate) backend_runtime: BackendOidcModeRuntime<MokaPendingAuthStateMetadataRedemptionStore>,
    pub(crate) oidc_client: Arc<OidcClient<MokaPendingOauthStore>>,
    pub(crate) access_token_substrate_runtime: AccessTokenSubstrateRuntime,
    pub(crate) oauth_resource_server_verifier: Option<Arc<OAuthResourceServerVerifier>>,
    pub(crate) external_base_url: Url,
}

impl std::fmt::Debug for SecurityDeptSourceRuntimeBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SecurityDeptSourceRuntimeBundle")
            .field("backend_runtime", &self.backend_runtime)
            .field("oidc_client", &"<opaque>")
            .field(
                "access_token_substrate_runtime",
                &self.access_token_substrate_runtime,
            )
            .field(
                "oauth_resource_server_verifier",
                &self
                    .oauth_resource_server_verifier
                    .as_ref()
                    .map(|_| "<opaque>"),
            )
            .field("external_base_url", &self.external_base_url)
            .finish()
    }
}

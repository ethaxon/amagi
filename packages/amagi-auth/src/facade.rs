use std::{collections::BTreeMap, sync::Arc};

use amagi_config::{ApiServerConfig, OidcIdentityClaimConfig};
use amagi_db::DatabaseService;
use amagi_securitydept::{
    AuthRuntime, BackendOidcModeAuthorizeQuery, BackendOidcModeMetadataRedemptionRequest,
    BackendOidcModeMetadataRedemptionResponse, BackendOidcModeRefreshPayload,
    BackendOidcModeUserInfoRequest, FrontendOidcModeConfigProjection, OidcCodeCallbackSearchParams,
    SecurityDeptHttpResponse, SecurityDeptOidcSourceRuntime, VerifiedBearerPrincipalFacts,
    VerifiedOidcUserInfo,
};
use serde::Serialize;
use serde_json::Value;

use crate::{
    audit::{AuthAuditWriter, SanitizedQueryEnvelope, sanitize_query_envelope},
    error::{AuthError, AuthResult},
    principal::{AmagiPrincipal, ExternalOidcIdentity, OidcIdentityClaim, PrincipalError},
    repository::{AccountBindingRecord, AccountBindingRepository, AccountBindingResolution},
};

#[derive(Debug, Clone)]
pub struct AuthFacadeService {
    config: Arc<ApiServerConfig>,
    runtime: AuthRuntime,
    bindings: AccountBindingRepository,
    audit: AuthAuditWriter,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendOidcConfigProjection {
    pub source: String,
    pub redirect_path: String,
    pub config_projection_path: String,
    #[serde(flatten)]
    pub frontend_oidc: FrontendOidcModeConfigProjection,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrontendCallbackShellResponse {
    pub source: String,
    pub message: &'static str,
    pub frontend_callback_path: String,
    pub config_projection_path: String,
    pub query: SanitizedQueryEnvelope,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrincipalView {
    pub auth_user_id: String,
    pub user_id: String,
    pub oidc_source: String,
    pub oidc_subject: String,
    pub oidc_identity_key: String,
    pub oidc_identity_claim: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PrincipalResolutionResult {
    Resolved { principal: PrincipalView },
    SkippedMissingClaim { claim_name: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthenticatedOidcUserInfoResponse {
    pub source: String,
    pub user_info: VerifiedOidcUserInfo,
    pub principal_resolution: PrincipalResolutionResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct BearerPrincipalResolution {
    pub source: String,
    pub facts: VerifiedBearerPrincipalFacts,
    pub principal: Option<BearerBoundPrincipalView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BearerBoundPrincipalView {
    pub auth_user_id: String,
    pub user_id: String,
    pub oidc_source: String,
    pub oidc_subject: String,
}

impl AuthFacadeService {
    pub fn new(
        config: Arc<ApiServerConfig>,
        runtime: AuthRuntime,
        database: DatabaseService,
    ) -> Self {
        Self::with_audit_writer(config, runtime, database, AuthAuditWriter::new())
    }

    pub fn with_audit_writer(
        config: Arc<ApiServerConfig>,
        runtime: AuthRuntime,
        database: DatabaseService,
        audit: AuthAuditWriter,
    ) -> Self {
        Self {
            config,
            runtime,
            bindings: AccountBindingRepository::new(database),
            audit,
        }
    }

    pub async fn frontend_config_projection(
        &self,
        source: &str,
    ) -> AuthResult<FrontendOidcConfigProjection> {
        let source_runtime = self.source_runtime(source)?;
        let projection = self.runtime.frontend_config_projection(source).await?;

        Ok(FrontendOidcConfigProjection {
            source: source_runtime.host.source_key.clone(),
            redirect_path: source_runtime.host.frontend_callback_path.clone(),
            config_projection_path: source_runtime.host.frontend_config_projection_path.clone(),
            frontend_oidc: projection,
        })
    }

    pub async fn oidc_start(
        &self,
        source: &str,
        query: &BackendOidcModeAuthorizeQuery,
    ) -> AuthResult<SecurityDeptHttpResponse> {
        let result = self
            .runtime
            .oidc_start(source, query)
            .await
            .map_err(Into::into);
        if result.is_ok() {
            let _ = self
                .audit
                .preview_start(source, query.post_auth_redirect_uri.is_some());
        }
        result
    }

    pub fn frontend_callback_shell(
        &self,
        source: &str,
        query: &BTreeMap<String, String>,
    ) -> AuthResult<FrontendCallbackShellResponse> {
        let source_runtime = self.source_runtime(source)?;

        Ok(FrontendCallbackShellResponse {
            source: source_runtime.host.source_key.clone(),
            message: "Frontend callback path is reserved for the browser app shell.",
            frontend_callback_path: source_runtime.host.frontend_callback_path.clone(),
            config_projection_path: source_runtime.host.frontend_config_projection_path.clone(),
            query: sanitize_query_envelope(query),
        })
    }

    pub async fn oidc_callback_body_return(
        &self,
        source: &str,
        query: &OidcCodeCallbackSearchParams,
    ) -> AuthResult<Value> {
        let query_envelope = BTreeMap::from_iter([
            ("code".to_owned(), query.code.clone()),
            ("state".to_owned(), query.state.clone().unwrap_or_default()),
        ]);
        let result = self
            .runtime
            .oidc_callback_body_return(
                source,
                OidcCodeCallbackSearchParams {
                    code: query.code.clone(),
                    state: query.state.clone(),
                },
            )
            .await
            .map_err(Into::into);
        match &result {
            Ok(_) => {
                let _ = self.audit.preview_callback_succeeded(
                    source,
                    &query_envelope,
                    "backend_callback_body",
                );
            }
            Err(error) => {
                let _ = self.audit.preview_callback_failed(
                    source,
                    &query_envelope,
                    "backend_callback_body",
                    error,
                );
            }
        }
        result
    }

    pub async fn oidc_callback_fragment_return(
        &self,
        source: &str,
        query: &OidcCodeCallbackSearchParams,
    ) -> AuthResult<SecurityDeptHttpResponse> {
        let query_envelope = BTreeMap::from_iter([
            ("code".to_owned(), query.code.clone()),
            ("state".to_owned(), query.state.clone().unwrap_or_default()),
        ]);
        let result = self
            .runtime
            .oidc_callback_fragment_return(
                source,
                OidcCodeCallbackSearchParams {
                    code: query.code.clone(),
                    state: query.state.clone(),
                },
            )
            .await
            .map_err(Into::into);
        match &result {
            Ok(_) => {
                let _ = self.audit.preview_callback_succeeded(
                    source,
                    &query_envelope,
                    "backend_callback_fragment",
                );
            }
            Err(error) => {
                let _ = self.audit.preview_callback_failed(
                    source,
                    &query_envelope,
                    "backend_callback_fragment",
                    error,
                );
            }
        }
        result
    }

    pub async fn oidc_refresh_body_return(
        &self,
        source: &str,
        payload: &BackendOidcModeRefreshPayload,
    ) -> AuthResult<Value> {
        let result = self
            .runtime
            .oidc_refresh_body_return(source, payload)
            .await
            .map_err(Into::into);
        match &result {
            Ok(_) => {
                let _ = self.audit.preview_refresh_succeeded(source);
            }
            Err(error) => {
                let _ = self.audit.preview_refresh_failed(source, error);
            }
        }
        result
    }

    pub async fn oidc_metadata_redeem(
        &self,
        source: &str,
        request: &BackendOidcModeMetadataRedemptionRequest,
    ) -> AuthResult<BackendOidcModeMetadataRedemptionResponse> {
        let result = match self.runtime.oidc_metadata_redeem(source, request).await {
            Ok(Some(response)) => Ok(response),
            Ok(None) => Err(AuthError::MetadataRedemptionNotFound {
                source_key: source.to_owned(),
            }),
            Err(error) => Err(AuthError::from(error)),
        };
        match &result {
            Ok(_) => {
                let _ = self.audit.preview_metadata_redeem_succeeded(source);
            }
            Err(error) => {
                let _ = self.audit.preview_metadata_redeem_failed(source, error);
            }
        }
        result
    }

    pub async fn oidc_user_info(
        &self,
        source: &str,
        request: &BackendOidcModeUserInfoRequest,
        access_token: Option<&str>,
    ) -> AuthResult<AuthenticatedOidcUserInfoResponse> {
        let access_token = match access_token {
            Some(access_token) => access_token,
            None => {
                let error = AuthError::MissingAccessToken;
                let _ = self.audit.preview_user_info_failed(source, &error);
                return Err(error);
            }
        };
        let user_info = match self
            .runtime
            .oidc_user_info(source, request, access_token)
            .await
        {
            Ok(user_info) => {
                let _ = self.audit.preview_user_info_succeeded(source, &user_info);
                user_info
            }
            Err(error) => {
                let error = AuthError::from(error);
                let _ = self.audit.preview_user_info_failed(source, &error);
                return Err(error);
            }
        };
        let principal_resolution = self
            .resolve_principal_from_verified_oidc_claims(source, &user_info)
            .await?;

        Ok(AuthenticatedOidcUserInfoResponse {
            source: source.to_owned(),
            user_info,
            principal_resolution,
        })
    }

    pub async fn resolve_principal(
        &self,
        source: &str,
        claims_snapshot: Value,
    ) -> AuthResult<AccountBindingResolution> {
        self.source_runtime(source)?;

        let identity_claim = self.oidc_identity_claim(source)?;
        let identity = ExternalOidcIdentity::new(source, identity_claim, claims_snapshot)?;

        self.bindings
            .resolve_or_create(&identity, &self.audit)
            .await
    }

    pub async fn resolve_principal_from_verified_oidc_claims(
        &self,
        source: &str,
        user_info: &VerifiedOidcUserInfo,
    ) -> AuthResult<PrincipalResolutionResult> {
        match self
            .resolve_principal(source, user_info.claims_snapshot())
            .await
        {
            Ok(binding) => Ok(PrincipalResolutionResult::Resolved {
                principal: PrincipalView::from(binding.principal()),
            }),
            Err(AuthError::Principal {
                source: PrincipalError::MissingClaim { claim_name },
            }) => Ok(PrincipalResolutionResult::SkippedMissingClaim { claim_name }),
            Err(error) => Err(error),
        }
    }

    pub async fn authenticate_bearer_principal(
        &self,
        source: &str,
        authorization_header: Option<&str>,
    ) -> AuthResult<Option<BearerPrincipalResolution>> {
        if authorization_header.is_none_or(|value| value.trim().is_empty()) {
            return Ok(None);
        }

        let facts = match self
            .runtime
            .authenticate_bearer(source, authorization_header)
            .await?
        {
            Some(facts) => facts,
            None => return Ok(None),
        };

        self.resolve_bearer_principal_from_facts(facts)
            .await
            .map(Some)
    }

    pub async fn resolve_bearer_principal_from_facts(
        &self,
        facts: VerifiedBearerPrincipalFacts,
    ) -> AuthResult<BearerPrincipalResolution> {
        self.source_runtime(&facts.source_key)?;

        let principal = match facts.subject.as_deref() {
            Some(subject) => self
                .bindings
                .lookup_by_oidc_subject(&facts.source_key, subject)
                .await?
                .map(BearerBoundPrincipalView::from),
            None => None,
        };

        Ok(BearerPrincipalResolution {
            source: facts.source_key.clone(),
            facts,
            principal,
        })
    }

    pub fn binding_repository(&self) -> &AccountBindingRepository {
        &self.bindings
    }
}

impl AuthFacadeService {
    fn source_runtime(&self, source: &str) -> AuthResult<&SecurityDeptOidcSourceRuntime> {
        self.runtime
            .securitydept
            .token_set
            .oidc_sources
            .get(source)
            .ok_or_else(|| AuthError::UnknownOidcSource {
                source_key: source.to_owned(),
            })
    }

    fn oidc_identity_claim(&self, source: &str) -> AuthResult<OidcIdentityClaim> {
        let source_config =
            self.config
                .oidc_sources
                .get(source)
                .ok_or_else(|| AuthError::UnknownOidcSource {
                    source_key: source.to_owned(),
                })?;

        Ok(match &source_config.oidc_identity_claim {
            OidcIdentityClaimConfig::Sub => OidcIdentityClaim::Sub,
            OidcIdentityClaimConfig::Email => OidcIdentityClaim::Email,
            OidcIdentityClaimConfig::Name => OidcIdentityClaim::Name,
            OidcIdentityClaimConfig::PreferredUsername => OidcIdentityClaim::PreferredUsername,
            OidcIdentityClaimConfig::CustomClaim { claim_name } => {
                OidcIdentityClaim::CustomClaim(claim_name.clone())
            }
        })
    }
}

impl From<&AmagiPrincipal> for PrincipalView {
    fn from(value: &AmagiPrincipal) -> Self {
        let external_identity = value.external_identity();

        Self {
            auth_user_id: value.auth_user_id().to_string(),
            user_id: value.user_id().to_string(),
            oidc_source: external_identity.source_key().to_owned(),
            oidc_subject: external_identity.oidc_subject().to_owned(),
            oidc_identity_key: external_identity.oidc_identity_key().to_owned(),
            oidc_identity_claim: external_identity.identity_claim().as_str().to_owned(),
        }
    }
}

impl From<AccountBindingRecord> for BearerBoundPrincipalView {
    fn from(value: AccountBindingRecord) -> Self {
        Self {
            auth_user_id: value.auth_user_id().to_string(),
            user_id: value.user_id().to_string(),
            oidc_source: value.oidc_source().to_owned(),
            oidc_subject: value.oidc_subject().to_owned(),
        }
    }
}

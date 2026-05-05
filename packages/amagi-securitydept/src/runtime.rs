use std::sync::Arc;

use amagi_config::{ApiServerConfig, TokenSetConfig};
use securitydept_core::{
    oidc::{MokaPendingOauthStore, OidcCodeCallbackSearchParams},
    token_set_context::{
        access_token_substrate::{
            AccessTokenSubstrateResourceService, AccessTokenSubstrateRuntime,
        },
        backend_oidc_mode::{
            BackendOidcModeAuthService, BackendOidcModeAuthorizeQuery,
            BackendOidcModeMetadataRedemptionRequest, BackendOidcModeMetadataRedemptionResponse,
            BackendOidcModeRefreshPayload, BackendOidcModeRuntime, BackendOidcModeUserInfoRequest,
            MokaPendingAuthStateMetadataRedemptionStore,
        },
        frontend_oidc_mode::{
            FrontendOidcModeConfigProjection, FrontendOidcModeRuntime, FrontendOidcModeService,
        },
    },
    utils::http::HttpResponse,
};
use url::Url;

use crate::{
    SecurityDeptError,
    model::{
        AuthRuntime, BackendOidcFacadePaths, ResolvedSecurityDeptSourceConfig,
        SecurityDeptAuthRuntime, SecurityDeptHttpResponse, SecurityDeptOidcSourceHostConfig,
        SecurityDeptOidcSourceRuntime, SecurityDeptSourceRuntimeBundle, TokenSetAuthConfig,
        TokenSetStoragePolicyRuntime, VerifiedBearerPrincipalFacts, VerifiedOidcUserInfo,
    },
    paths::{
        backend_oidc_redirect_path, frontend_oidc_config_projection_path,
        frontend_oidc_redirect_path,
    },
    resolver::build_securitydept_source_config,
};

impl AuthRuntime {
    pub fn from_api_config(config: &ApiServerConfig) -> Self {
        Self {
            securitydept: SecurityDeptAuthRuntime {
                token_set: TokenSetAuthConfig::from_api_config(config),
            },
        }
    }

    pub async fn frontend_config_projection(
        &self,
        source: &str,
    ) -> Result<FrontendOidcModeConfigProjection, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let resolved = source_runtime
            .resolved
            .as_ref()
            .ok_or_else(|| source_runtime.config_error(source))?;
        let service = FrontendOidcModeService::new(FrontendOidcModeRuntime::new(
            resolved.frontend_oidc.clone(),
        ));

        service
            .config_projection()
            .await
            .map_err(|io_error| SecurityDeptError::frontend_projection(source, &io_error))
    }

    pub async fn oidc_start(
        &self,
        source: &str,
        query: &BackendOidcModeAuthorizeQuery,
    ) -> Result<SecurityDeptHttpResponse, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .login(&bundle.external_base_url, query)
            .await
            .map(Into::into)
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn oidc_callback_body_return(
        &self,
        source: &str,
        params: OidcCodeCallbackSearchParams,
    ) -> Result<serde_json::Value, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .callback_body_return(&bundle.external_base_url, params)
            .await
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn oidc_callback_fragment_return(
        &self,
        source: &str,
        params: OidcCodeCallbackSearchParams,
    ) -> Result<SecurityDeptHttpResponse, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .callback_fragment_return(&bundle.external_base_url, params, None)
            .await
            .map(Into::into)
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn oidc_refresh_body_return(
        &self,
        source: &str,
        payload: &BackendOidcModeRefreshPayload,
    ) -> Result<serde_json::Value, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .refresh_body_return(payload)
            .await
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn oidc_metadata_redeem(
        &self,
        source: &str,
        payload: &BackendOidcModeMetadataRedemptionRequest,
    ) -> Result<Option<BackendOidcModeMetadataRedemptionResponse>, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .redeem_metadata(payload)
            .await
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn oidc_user_info(
        &self,
        source: &str,
        request: &BackendOidcModeUserInfoRequest,
        access_token: &str,
    ) -> Result<VerifiedOidcUserInfo, SecurityDeptError> {
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let service = backend_auth_service(source_runtime, &bundle)?;

        service
            .user_info(request, access_token)
            .await
            .map(|response| VerifiedOidcUserInfo {
                subject: response.subject,
                display_name: response.display_name,
                picture: response.picture,
                issuer: response.issuer,
                claims: response.claims.map(|claims| claims.into_iter().collect()),
            })
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    pub async fn authenticate_bearer(
        &self,
        source: &str,
        authorization_header: Option<&str>,
    ) -> Result<Option<VerifiedBearerPrincipalFacts>, SecurityDeptError> {
        let Some(authorization_header) =
            authorization_header.filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        let source_runtime = self.source_runtime(source)?;
        let bundle = self.source_bundle(source_runtime, source).await?;
        let verifier = bundle
            .oauth_resource_server_verifier
            .as_ref()
            .ok_or_else(|| SecurityDeptError::bearer_verifier_unavailable(source))?;
        let service = AccessTokenSubstrateResourceService::new(
            &bundle.access_token_substrate_runtime,
            verifier,
        );

        service
            .authenticate_authorization_header(Some(authorization_header))
            .await
            .map(|principal| {
                principal.map(|principal| {
                    let mut facts = VerifiedBearerPrincipalFacts::from(principal);
                    facts.source_key = source.to_owned();
                    facts
                })
            })
            .map_err(|error| SecurityDeptError::from_securitydept(source, &error))
    }

    fn source_runtime(
        &self,
        source: &str,
    ) -> Result<&SecurityDeptOidcSourceRuntime, SecurityDeptError> {
        self.securitydept
            .token_set
            .oidc_sources
            .get(source)
            .ok_or_else(|| SecurityDeptError::unknown_source(source))
    }

    async fn source_bundle(
        &self,
        source_runtime: &SecurityDeptOidcSourceRuntime,
        source: &str,
    ) -> Result<Arc<SecurityDeptSourceRuntimeBundle>, SecurityDeptError> {
        source_runtime
            .runtime_bundle
            .get_or_init(|| {
                let source = source.to_owned();
                let resolved = source_runtime
                    .resolved
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| source_runtime.config_error(source.as_str()));
                async move {
                    let resolved = resolved?;
                    build_runtime_bundle(source.as_str(), resolved)
                        .await
                        .map(Arc::new)
                }
            })
            .await
            .clone()
    }
}

impl TokenSetAuthConfig {
    pub fn from_api_config(config: &ApiServerConfig) -> Self {
        let oidc_sources = config
            .oidc_sources
            .iter()
            .map(|(source_key, source_config)| {
                let resolved = build_securitydept_source_config(config, source_key, source_config);
                let (resolved, source_resolution_error) = match resolved {
                    Ok(resolved) => (Some(Arc::new(resolved)), None),
                    Err(error) => (None, Some(error)),
                };

                (
                    source_key.clone(),
                    SecurityDeptOidcSourceRuntime {
                        host: SecurityDeptOidcSourceHostConfig {
                            source_key: source_key.clone(),
                            backend_callback_path: backend_oidc_redirect_path(source_key),
                            frontend_callback_path: frontend_oidc_redirect_path(source_key),
                            frontend_config_projection_path: frontend_oidc_config_projection_path(
                                source_key,
                            ),
                        },
                        resolved,
                        source_resolution_error,
                        runtime_bundle: Arc::new(tokio::sync::OnceCell::new()),
                    },
                )
            })
            .collect();

        Self {
            default_oidc_source: config.default_oidc_source.clone(),
            oidc_sources,
            facade_paths: BackendOidcFacadePaths {
                start: config.token_set.facade_paths.start.clone(),
                callback: config.token_set.facade_paths.callback.clone(),
            },
            securitydept_backend_oidc_surface: securitydept_backend_oidc_surface(),
            securitydept_frontend_oidc_surface: securitydept_frontend_oidc_surface(),
            securitydept_access_token_substrate_surface:
                securitydept_access_token_substrate_surface(),
            storage_policy: TokenSetStoragePolicyRuntime::from_config(&config.token_set),
            extension_session_binding_required: config.token_set.browser_client_binding_required,
            cookie_session_dashboard_only: config.token_set.cookie_session_dashboard_only,
        }
    }
}

impl SecurityDeptOidcSourceRuntime {
    fn config_error(&self, source: &str) -> SecurityDeptError {
        self.source_resolution_error.clone().unwrap_or_else(|| {
            SecurityDeptError::config(
                source,
                "auth.oidc.source_resolution_missing",
                format!(
                    "OIDC source `{source}` does not have a resolved SecurityDept config bundle."
                ),
            )
        })
    }
}

fn securitydept_backend_oidc_surface() -> &'static str {
    std::any::type_name::<crate::SecurityDeptBackendOidcConfig>()
}

fn securitydept_frontend_oidc_surface() -> &'static str {
    std::any::type_name::<crate::SecurityDeptFrontendOidcConfig>()
}

fn securitydept_access_token_substrate_surface() -> &'static str {
    std::any::type_name::<crate::SecurityDeptAccessTokenSubstrateConfig>()
}

impl TokenSetStoragePolicyRuntime {
    fn from_config(config: &TokenSetConfig) -> Self {
        Self {
            pending_state_store: config.storage_policy.pending_state_store.clone(),
            refresh_token_material: config.storage_policy.refresh_token_material.clone(),
        }
    }
}

async fn build_runtime_bundle(
    source: &str,
    resolved: Arc<ResolvedSecurityDeptSourceConfig>,
) -> Result<SecurityDeptSourceRuntimeBundle, SecurityDeptError> {
    let external_base_url = Url::parse(&resolved.external_base_url).map_err(|error| {
        SecurityDeptError::config(
            source,
            "auth.oidc.external_base_url_invalid",
            format!(
                "SecurityDept runtime could not parse external_base_url `{}` for source \
                 `{source}`: {error}",
                resolved.external_base_url
            ),
        )
    })?;
    let (backend_runtime, oidc_client) = BackendOidcModeRuntime::from_resolved_config::<
        MokaPendingOauthStore,
    >(Some(&resolved.backend_oidc))
    .await
    .map_err(|error| SecurityDeptError::from_securitydept(source, &error))?;
    let oidc_client = oidc_client.ok_or_else(|| SecurityDeptError::missing_oidc_client(source))?;
    let (access_token_substrate_runtime, oauth_resource_server_verifier) =
        AccessTokenSubstrateRuntime::from_resolved_config(&resolved.access_token_substrate)
            .await
            .map_err(|error| {
                SecurityDeptError::runtime_build(
                    source,
                    "auth.oidc.resource_server_runtime_build_failed",
                    error.to_string(),
                )
            })?;

    Ok(SecurityDeptSourceRuntimeBundle {
        backend_runtime,
        oidc_client,
        access_token_substrate_runtime,
        oauth_resource_server_verifier,
        external_base_url,
    })
}

fn backend_auth_service<'a>(
    source_runtime: &'a SecurityDeptOidcSourceRuntime,
    bundle: &'a SecurityDeptSourceRuntimeBundle,
) -> Result<
    BackendOidcModeAuthService<
        'a,
        MokaPendingOauthStore,
        MokaPendingAuthStateMetadataRedemptionStore,
    >,
    SecurityDeptError,
> {
    let callback_path = source_runtime
        .resolved
        .as_ref()
        .map(|_| source_runtime.host.backend_callback_path.as_str())
        .ok_or_else(|| source_runtime.config_error(&source_runtime.host.source_key))?;

    Ok(BackendOidcModeAuthService::new(
        bundle.oidc_client.as_ref(),
        &bundle.backend_runtime,
        callback_path,
    ))
}

impl From<HttpResponse> for SecurityDeptHttpResponse {
    fn from(value: HttpResponse) -> Self {
        Self {
            status: value.status.as_u16(),
            headers: value
                .headers
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                })
                .collect(),
        }
    }
}

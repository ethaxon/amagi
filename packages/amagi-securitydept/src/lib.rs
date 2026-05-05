mod error;
mod model;
mod paths;
mod resolver;
mod runtime;

pub use error::SecurityDeptError;
pub use model::{
    AuthRuntime, BackendOidcFacadePaths, SecurityDeptAccessTokenSubstrateConfig,
    SecurityDeptAuthRuntime, SecurityDeptBackendOidcConfig, SecurityDeptFrontendOidcConfig,
    SecurityDeptHttpResponse, SecurityDeptOidcSharedConfig, SecurityDeptOidcSourceHostConfig,
    SecurityDeptOidcSourceRuntime, TokenSetAuthConfig, TokenSetStoragePolicyRuntime,
    VerifiedBearerPrincipalFacts, VerifiedOidcUserInfo,
};
pub use paths::{
    backend_oidc_redirect_path, frontend_oidc_config_projection_path, frontend_oidc_redirect_path,
};
pub use securitydept_core::{
    oidc::OidcCodeCallbackSearchParams,
    token_set_context::{
        backend_oidc_mode::{
            BackendOidcModeAuthorizeQuery, BackendOidcModeMetadataRedemptionRequest,
            BackendOidcModeMetadataRedemptionResponse, BackendOidcModeRefreshPayload,
            BackendOidcModeUserInfoRequest,
        },
        frontend_oidc_mode::FrontendOidcModeConfigProjection,
    },
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use amagi_config::{
        AccessTokenSubstrateSourceConfig, ApiServerConfig, BackendOidcSourceConfig,
        OidcSharedSourceConfig, OidcSourceConfig, SecretString,
    };
    use securitydept_core::token_set_context::{
        access_token_substrate::{TokenPropagation, TokenPropagatorConfig},
        frontend_oidc_mode::UnsafeFrontendClientSecret,
    };

    use super::*;

    fn sample_config() -> ApiServerConfig {
        let mut config = ApiServerConfig {
            default_oidc_source: Some("primary".to_owned()),
            ..ApiServerConfig::default()
        };

        config.oidc_sources = BTreeMap::from([
            (
                "primary".to_owned(),
                OidcSourceConfig {
                    oidc: OidcSharedSourceConfig {
                        remote: securitydept_core::oauth_provider::OAuthProviderRemoteConfig {
                            issuer_url: Some("https://issuer.primary".to_owned()),
                            well_known_url: Some(
                                "https://issuer.primary/.well-known/openid-configuration"
                                    .to_owned(),
                            ),
                            ..Default::default()
                        },
                        client_id: Some("interactive-client".to_owned()),
                        client_secret: Some(SecretString::new("shared-secret")),
                        scopes: vec!["openid".to_owned(), "email".to_owned()],
                        ..OidcSharedSourceConfig::default()
                    },
                    backend_oidc: BackendOidcSourceConfig {
                        oidc_client: securitydept_core::token_set_context::cross_mode_config::OptionalTokenSetOidcSharedIntersectionConfig {
                            client_secret: Some(SecretString::new("backend-secret")),
                            ..Default::default()
                        },
                        ..BackendOidcSourceConfig::default()
                    },
                    access_token_substrate: AccessTokenSubstrateSourceConfig {
                        resource_server: securitydept_core::oauth_resource_server::OAuthResourceServerConfig {
                            introspection: Some(
                                securitydept_core::oauth_resource_server::OAuthResourceServerIntrospectionConfig {
                                    introspection_url: Some(
                                        "https://issuer.primary/introspect".to_owned(),
                                    ),
                                    ..Default::default()
                                },
                            ),
                            audiences: vec!["api://amagi".to_owned()],
                            required_scopes: vec!["amagi.sync".to_owned()],
                            ..Default::default()
                        },
                        ..AccessTokenSubstrateSourceConfig::default()
                    },
                    ..OidcSourceConfig::default()
                },
            ),
            (
                "secondary".to_owned(),
                OidcSourceConfig {
                    oidc: OidcSharedSourceConfig {
                        remote: securitydept_core::oauth_provider::OAuthProviderRemoteConfig {
                            issuer_url: Some("https://issuer.secondary".to_owned()),
                            ..Default::default()
                        },
                        client_id: Some("secondary-client".to_owned()),
                        ..OidcSharedSourceConfig::default()
                    },
                    ..OidcSourceConfig::default()
                },
            ),
        ]);

        config
    }

    #[test]
    fn token_set_runtime_keeps_multiple_oidc_sources() {
        let runtime = AuthRuntime::from_api_config(&sample_config());

        assert_eq!(
            runtime
                .securitydept
                .token_set
                .default_oidc_source
                .as_deref(),
            Some("primary")
        );
        assert_eq!(runtime.securitydept.token_set.oidc_sources.len(), 2);
        assert!(
            runtime
                .securitydept
                .token_set
                .oidc_sources
                .contains_key("secondary")
        );
    }

    #[test]
    fn backend_oidc_and_resource_server_resolve_separately() {
        let runtime = AuthRuntime::from_api_config(&sample_config());
        let source = &runtime.securitydept.token_set.oidc_sources["primary"];
        let resolved = source
            .resolved
            .as_ref()
            .expect("resolved config is present");

        assert_eq!(
            resolved.backend_oidc.oidc_client.client_id,
            "interactive-client"
        );
        assert_eq!(
            resolved
                .backend_oidc
                .oidc_client
                .client_secret
                .as_ref()
                .expect("backend secret")
                .expose_secret(),
            "backend-secret"
        );
        assert_eq!(
            source.host.frontend_config_projection_path,
            "/api/auth/token-set/oidc/source/primary/config"
        );
        assert_eq!(
            resolved.access_token_substrate.resource_server.audiences,
            vec!["api://amagi"]
        );
        assert_eq!(
            resolved
                .access_token_substrate
                .resource_server
                .required_scopes,
            vec!["amagi.sync".to_owned()]
        );
    }

    #[test]
    fn resolved_paths_use_external_base_url() {
        let runtime = AuthRuntime::from_api_config(&sample_config());
        let source = &runtime.securitydept.token_set.oidc_sources["primary"];
        let projection = tokio::runtime::Runtime::new()
            .expect("runtime builds")
            .block_on(runtime.frontend_config_projection("primary"))
            .expect("frontend projection resolves");

        assert_eq!(
            source.host.backend_callback_path,
            "/api/auth/token-set/oidc/source/primary/callback"
        );
        assert_eq!(
            source.host.frontend_callback_path,
            "/auth/token-set/oidc/source/primary/callback"
        );
        assert_eq!(
            source.host.frontend_config_projection_path,
            "/api/auth/token-set/oidc/source/primary/config"
        );
        assert_eq!(
            projection.redirect_url,
            "http://127.0.0.1:7800/auth/token-set/oidc/source/primary/callback"
        );
    }

    #[test]
    fn surface_markers_use_securitydept_typed_configs() {
        let runtime = AuthRuntime::from_api_config(&sample_config());

        assert!(
            runtime
                .securitydept
                .token_set
                .securitydept_backend_oidc_surface
                .contains("BackendOidcModeConfig")
        );
        assert!(
            runtime
                .securitydept
                .token_set
                .securitydept_access_token_substrate_surface
                .contains("AccessTokenSubstrateConfig")
        );
    }

    #[tokio::test]
    async fn frontend_projection_is_available_without_runtime_build() {
        let runtime = AuthRuntime::from_api_config(&sample_config());
        let projection = runtime
            .frontend_config_projection("primary")
            .await
            .expect("frontend projection resolves");

        assert_eq!(projection.client_id, "interactive-client");
        assert!(projection.client_secret.is_none());
    }

    #[test]
    fn runtime_rejects_enabled_token_propagation_even_without_config_loader() {
        let mut config = sample_config();
        config
            .oidc_sources
            .get_mut("primary")
            .expect("source exists")
            .access_token_substrate
            .token_propagation = TokenPropagation::Enabled {
            config: TokenPropagatorConfig::default(),
        };

        let runtime = AuthRuntime::from_api_config(&config);
        let source = &runtime.securitydept.token_set.oidc_sources["primary"];
        let error = source
            .source_resolution_error
            .as_ref()
            .expect("enabled propagation should be rejected during resolver build");

        assert!(source.resolved.is_none());
        assert_eq!(error.code(), "auth.oidc.resource_server_config_invalid");
        assert!(error.message().contains("disabled_by_host"));
        assert_eq!(error.source_key(), Some("primary"));
    }

    #[test]
    fn runtime_rejects_unsafe_frontend_client_secret_even_without_config_loader() {
        let mut config = sample_config();
        config
            .oidc_sources
            .get_mut("primary")
            .expect("source exists")
            .frontend_oidc
            .unsafe_frontend_client_secret = Some(UnsafeFrontendClientSecret::Enabled);

        let runtime = AuthRuntime::from_api_config(&config);
        let source = &runtime.securitydept.token_set.oidc_sources["primary"];
        let error = source
            .source_resolution_error
            .as_ref()
            .expect("unsafe frontend client_secret should be rejected during resolver build");

        assert!(source.resolved.is_none());
        assert_eq!(error.code(), "auth.oidc.frontend_config_invalid");
        assert!(error.message().contains("unsafe_frontend_client_secret"));
        assert!(error.message().contains("disabled_by_host"));
        assert_eq!(error.source_key(), Some("primary"));
    }
}

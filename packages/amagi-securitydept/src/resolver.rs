use amagi_config::{ApiServerConfig, OidcSharedSourceConfig, OidcSourceConfig};
use securitydept_core::{
    oidc::MokaPendingOauthStoreConfig,
    token_set_context::{
        access_token_substrate::{
            AccessTokenSubstrateConfig, AccessTokenSubstrateConfigSource,
            AccessTokenSubstrateConfigValidationError, AccessTokenSubstrateConfigValidator,
            TokenPropagation,
        },
        backend_oidc_mode::{
            BackendOidcModeConfigSource, BackendOidcModeFixedRedirectUriValidator,
            MokaPendingAuthStateMetadataRedemptionConfig,
        },
        cross_mode_config::TokenSetOidcSharedUnionConfig,
        frontend_oidc_mode::{
            FrontendOidcModeConfig, FrontendOidcModeConfigSource,
            FrontendOidcModeConfigValidationError, FrontendOidcModeConfigValidator,
            FrontendOidcModeFixedRedirectUriValidator,
        },
    },
};

use crate::{
    SecurityDeptError,
    model::{
        ResolvedSecurityDeptSourceConfig, SecurityDeptAccessTokenSubstrateConfig,
        SecurityDeptBackendOidcConfig, SecurityDeptFrontendOidcConfig,
        SecurityDeptOidcSharedConfig,
    },
    paths::{backend_oidc_redirect_path, frontend_oidc_redirect_path, qualify_path},
};

type SecurityDeptCrossModeSharedConfig = TokenSetOidcSharedUnionConfig<
    MokaPendingOauthStoreConfig,
    MokaPendingAuthStateMetadataRedemptionConfig,
>;

#[derive(Debug, Clone, Copy, Default)]
struct AccessTokenSubstrateHostPolicyValidator;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontendOidcModeHostPolicyValidator {
    redirect_url: String,
}

impl FrontendOidcModeHostPolicyValidator {
    fn new(redirect_url: impl Into<String>) -> Self {
        Self {
            redirect_url: redirect_url.into(),
        }
    }
}

impl FrontendOidcModeConfigValidator for FrontendOidcModeHostPolicyValidator {
    fn validate_raw_frontend_oidc_mode_config(
        &self,
        config: &FrontendOidcModeConfig,
    ) -> Result<(), FrontendOidcModeConfigValidationError> {
        FrontendOidcModeFixedRedirectUriValidator::new(self.redirect_url.as_str())
            .validate_raw_frontend_oidc_mode_config(config)?;

        if config
            .capabilities
            .unsafe_frontend_client_secret
            .is_enabled()
        {
            return Err(FrontendOidcModeConfigValidationError::new(
                "unsafe_frontend_client_secret",
                "disabled_by_host",
                "frontend client_secret exposure is disabled by the host",
            ));
        }

        Ok(())
    }
}

impl AccessTokenSubstrateConfigValidator for AccessTokenSubstrateHostPolicyValidator {
    fn validate_raw_access_token_substrate_config(
        &self,
        config: &AccessTokenSubstrateConfig,
    ) -> Result<(), AccessTokenSubstrateConfigValidationError> {
        if matches!(config.token_propagation, TokenPropagation::Enabled { .. }) {
            return Err(AccessTokenSubstrateConfigValidationError::new(
                "token_propagation",
                "disabled_by_host",
                "token propagation is disabled by the host",
            ));
        }

        Ok(())
    }
}

pub(crate) fn build_securitydept_source_config(
    config: &ApiServerConfig,
    source_key: &str,
    source_config: &OidcSourceConfig,
) -> Result<ResolvedSecurityDeptSourceConfig, SecurityDeptError> {
    let shared_oidc = build_shared_oidc(&source_config.oidc);
    let cross_mode_shared = build_cross_mode_shared(source_config);
    let backend_redirect_url = qualify_path(
        &config.external_base_url,
        &backend_oidc_redirect_path(source_key),
    );
    let frontend_redirect_url = qualify_path(
        &config.external_base_url,
        &frontend_oidc_redirect_path(source_key),
    );
    let backend_oidc = build_backend_oidc(source_config, &cross_mode_shared);
    let frontend_oidc = build_frontend_oidc(source_config, &cross_mode_shared);
    let access_token_substrate = build_access_token_substrate(source_config);

    let backend_oidc = {
        let validator =
            BackendOidcModeFixedRedirectUriValidator::new(backend_redirect_url.as_str());
        let mut resolved = backend_oidc
            .resolve_all_with_validator(&shared_oidc, &validator)
            .map_err(|source| {
                SecurityDeptError::config(
                    source_key,
                    "auth.oidc.backend_config_invalid",
                    source.to_string(),
                )
            })?;
        resolved.oidc_client.redirect_url = backend_redirect_url;
        resolved
    };

    let frontend_oidc = {
        let validator = FrontendOidcModeHostPolicyValidator::new(frontend_redirect_url.as_str());
        let mut resolved = frontend_oidc
            .resolve_all_with_validator(&shared_oidc, &validator)
            .map_err(|source| {
                SecurityDeptError::config(
                    source_key,
                    "auth.oidc.frontend_config_invalid",
                    source.to_string(),
                )
            })?;
        resolved.oidc_client.redirect_url = frontend_redirect_url;
        resolved
    };

    Ok(ResolvedSecurityDeptSourceConfig {
        _shared_oidc: shared_oidc.clone(),
        backend_oidc,
        frontend_oidc,
        access_token_substrate: access_token_substrate
            .resolve_all_with_validator(
                Some(&shared_oidc),
                &AccessTokenSubstrateHostPolicyValidator,
            )
            .map_err(|source| {
                SecurityDeptError::config(
                    source_key,
                    "auth.oidc.resource_server_config_invalid",
                    source.to_string(),
                )
            })?,
        external_base_url: config.external_base_url.clone(),
    })
}

fn build_shared_oidc(oidc: &OidcSharedSourceConfig) -> SecurityDeptOidcSharedConfig {
    SecurityDeptOidcSharedConfig {
        remote: oidc.remote.clone(),
        client_id: oidc.client_id.clone(),
        client_secret: oidc.client_secret.clone(),
        required_scopes: oidc.required_scopes.clone(),
    }
}

fn build_cross_mode_shared(source_config: &OidcSourceConfig) -> SecurityDeptCrossModeSharedConfig {
    SecurityDeptCrossModeSharedConfig {
        oidc_client: source_config.oidc.clone(),
        ..Default::default()
    }
}

fn build_backend_oidc(
    source_config: &OidcSourceConfig,
    cross_mode_shared: &SecurityDeptCrossModeSharedConfig,
) -> SecurityDeptBackendOidcConfig {
    cross_mode_shared.compose_backend_config(&source_config.backend_oidc)
}

fn build_frontend_oidc(
    source_config: &OidcSourceConfig,
    cross_mode_shared: &SecurityDeptCrossModeSharedConfig,
) -> SecurityDeptFrontendOidcConfig {
    cross_mode_shared.compose_frontend_config(&source_config.frontend_oidc)
}

fn build_access_token_substrate(
    source_config: &OidcSourceConfig,
) -> SecurityDeptAccessTokenSubstrateConfig {
    source_config.access_token_substrate.clone()
}

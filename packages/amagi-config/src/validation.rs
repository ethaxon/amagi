use securitydept_core::token_set_context::{
    access_token_substrate::TokenPropagation, frontend_oidc_mode::UnsafeFrontendClientSecret,
};

use crate::{
    error::{ConfigError, ConfigResult},
    model::{ApiServerConfig, OidcIdentityClaimConfig},
};

impl ApiServerConfig {
    pub(crate) fn validate(&self) -> ConfigResult<()> {
        if self.token_set.facade_paths.start != "/api/auth/token-set/oidc/source/{source}/start" {
            return Err(ConfigError::Invalid {
                message: "token_set.facade_paths.start is fixed to \
                          /api/auth/token-set/oidc/source/{source}/start"
                    .to_owned(),
            });
        }

        if self.token_set.facade_paths.callback
            != "/api/auth/token-set/oidc/source/{source}/callback"
        {
            return Err(ConfigError::Invalid {
                message: "token_set.facade_paths.callback is fixed to \
                          /api/auth/token-set/oidc/source/{source}/callback"
                    .to_owned(),
            });
        }

        if let Some(default_source) = &self.default_oidc_source
            && !self.oidc_sources.contains_key(default_source)
        {
            return Err(ConfigError::Invalid {
                message: format!(
                    "default_oidc_source `{default_source}` does not exist in oidc_sources"
                ),
            });
        }

        for (source_key, source_config) in &self.oidc_sources {
            if source_key.trim().is_empty() {
                return Err(ConfigError::Invalid {
                    message: "oidc_sources contains an empty source key".to_owned(),
                });
            }

            if source_config.oidc.redirect_url.is_some() {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.oidc.redirect_url is fixed by amagi and must \
                         not be configured"
                    ),
                });
            }

            if source_config
                .backend_oidc
                .oidc_client
                .redirect_url
                .is_some()
            {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.backend_oidc.redirect_url is fixed by amagi \
                         and must not be configured"
                    ),
                });
            }

            if source_config
                .frontend_oidc
                .oidc_client
                .redirect_url
                .is_some()
            {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.frontend_oidc.redirect_url is fixed by amagi \
                         and must not be configured"
                    ),
                });
            }

            if matches!(
                source_config.frontend_oidc.unsafe_frontend_client_secret,
                Some(UnsafeFrontendClientSecret::Enabled)
            ) {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.frontend_oidc.unsafe_frontend_client_secret is \
                         disabled by amagi"
                    ),
                });
            }

            if source_config.backend_oidc.pending_store.is_some() {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.backend_oidc.pending_store is owned by amagi \
                         token_set.storage_policy and must not be configured"
                    ),
                });
            }

            if !matches!(
                source_config.access_token_substrate.token_propagation,
                TokenPropagation::Disabled
            ) {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.access_token_substrate.token_propagation is \
                         disabled by amagi; remove forwarding flags"
                    ),
                });
            }

            if let OidcIdentityClaimConfig::CustomClaim { claim_name } =
                &source_config.oidc_identity_claim
                && claim_name.trim().is_empty()
            {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "oidc_sources.{source_key}.oidc_identity_claim.claim_name must not be \
                         empty"
                    ),
                });
            }
        }

        Ok(())
    }
}

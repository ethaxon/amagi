use std::{borrow::Cow, collections::BTreeMap, fmt, net::SocketAddr};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
pub use securitydept_core::oidc::SecretString;
use securitydept_core::{
    oidc::MokaPendingOauthStoreConfig,
    token_set_context::{
        access_token_substrate::AccessTokenSubstrateConfig,
        backend_oidc_mode::MokaPendingAuthStateMetadataRedemptionConfig,
        cross_mode_config::{
            BackendOidcModeOverrideConfig, FrontendOidcModeOverrideConfig,
            TokenSetOidcSharedIntersectionConfig,
        },
    },
};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::error::{ConfigError, ConfigResult};

pub const DEFAULT_OIDC_SOURCE_KEY: &str = "default";

pub type OidcSharedSourceConfig = TokenSetOidcSharedIntersectionConfig;
pub type BackendOidcSourceConfig = BackendOidcModeOverrideConfig<
    MokaPendingOauthStoreConfig,
    MokaPendingAuthStateMetadataRedemptionConfig,
>;
pub type FrontendOidcSourceConfig = FrontendOidcModeOverrideConfig;
pub type AccessTokenSubstrateSourceConfig = AccessTokenSubstrateConfig;

#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    pub server: ServerConfig,
    pub external_base_url: String,
    pub default_oidc_source: Option<String>,
    pub oidc_sources: BTreeMap<String, OidcSourceConfig>,
    pub token_set: TokenSetConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: Option<SecretString>,
    pub auto_migrate: BooleanLike,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TokenSetConfig {
    pub facade_paths: BackendOidcFacadePathsConfig,
    pub storage_policy: TokenSetStoragePolicyConfig,
    pub browser_client_binding_required: bool,
    pub cookie_session_dashboard_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BackendOidcFacadePathsConfig {
    pub start: String,
    pub callback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TokenSetStoragePolicyConfig {
    pub pending_state_store: String,
    pub refresh_token_material: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OidcSourceConfig {
    pub oidc: OidcSharedSourceConfig,
    pub backend_oidc: BackendOidcSourceConfig,
    pub frontend_oidc: FrontendOidcSourceConfig,
    pub access_token_substrate: AccessTokenSubstrateSourceConfig,
    pub oidc_identity_claim: OidcIdentityClaimConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum OidcIdentityClaimConfig {
    #[default]
    Sub,
    Email,
    Name,
    PreferredUsername,
    CustomClaim {
        claim_name: String,
    },
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BooleanLike(pub(crate) bool);

impl BooleanLike {
    pub(crate) fn from_str(value: &str) -> Option<bool> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
}

impl fmt::Debug for BooleanLike {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

impl From<BooleanLike> for bool {
    fn from(value: BooleanLike) -> Self {
        value.0
    }
}

impl JsonSchema for BooleanLike {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "BooleanLike".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                { "type": "boolean" },
                { "type": "integer", "enum": [0, 1] },
                {
                    "type": "string",
                    "enum": ["true", "false", "yes", "no", "on", "off", "1", "0"]
                }
            ]
        })
    }
}

impl<'de> Deserialize<'de> for BooleanLike {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bool(bool),
            Signed(i64),
            Unsigned(u64),
            String(String),
        }

        let repr = Repr::deserialize(deserializer)?;
        let value = match repr {
            Repr::Bool(value) => value,
            Repr::Signed(value) => match value {
                0 => false,
                1 => true,
                _ => {
                    return Err(de::Error::custom(format!(
                        "invalid boolean-like integer `{value}`"
                    )));
                }
            },
            Repr::Unsigned(value) => match value {
                0 => false,
                1 => true,
                _ => {
                    return Err(de::Error::custom(format!(
                        "invalid boolean-like integer `{value}`"
                    )));
                }
            },
            Repr::String(value) => BooleanLike::from_str(&value).ok_or_else(|| {
                de::Error::custom(format!("invalid boolean-like value `{value}`"))
            })?,
        };

        Ok(Self(value))
    }
}

impl ApiServerConfig {
    pub fn bind_addr(&self) -> ConfigResult<SocketAddr> {
        format!("{}:{}", self.server.host, self.server.port)
            .parse::<SocketAddr>()
            .map_err(|source| ConfigError::Invalid {
                message: format!(
                    "invalid API bind address `{}:{}`: {source}",
                    self.server.host, self.server.port
                ),
            })
    }
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        let server = ServerConfig::default();

        Self {
            server: server.clone(),
            external_base_url: default_external_base_url(&server),
            default_oidc_source: None,
            oidc_sources: BTreeMap::new(),
            token_set: TokenSetConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 7800,
        }
    }
}

impl Default for TokenSetConfig {
    fn default() -> Self {
        Self {
            facade_paths: BackendOidcFacadePathsConfig::default(),
            storage_policy: TokenSetStoragePolicyConfig::default(),
            browser_client_binding_required: true,
            cookie_session_dashboard_only: true,
        }
    }
}

impl Default for BackendOidcFacadePathsConfig {
    fn default() -> Self {
        Self {
            start: "/api/auth/token-set/oidc/source/{source}/start".to_owned(),
            callback: "/api/auth/token-set/oidc/source/{source}/callback".to_owned(),
        }
    }
}

impl Default for TokenSetStoragePolicyConfig {
    fn default() -> Self {
        Self {
            pending_state_store: "server-moka".to_owned(),
            refresh_token_material: "sealed".to_owned(),
        }
    }
}

impl Default for OidcSourceConfig {
    fn default() -> Self {
        Self {
            oidc: OidcSharedSourceConfig {
                pkce_enabled: true,
                ..OidcSharedSourceConfig::default()
            },
            backend_oidc: BackendOidcSourceConfig::default(),
            frontend_oidc: FrontendOidcSourceConfig::default(),
            access_token_substrate: AccessTokenSubstrateSourceConfig::default(),
            oidc_identity_claim: OidcIdentityClaimConfig::default(),
        }
    }
}

pub(crate) fn default_external_base_url(server: &ServerConfig) -> String {
    format!("http://{}:{}", server.host, server.port)
}

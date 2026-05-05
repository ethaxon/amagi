use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::Deserialize;

use crate::model::{
    ApiServerConfig, DatabaseConfig, OidcSourceConfig, ServerConfig, TokenSetConfig,
};

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct ApiServerConfigInput {
    pub server: ServerConfig,
    pub external_base_url: Option<String>,
    pub default_oidc_source: Option<String>,
    pub oidc_sources: BTreeMap<String, OidcSourceConfig>,
    pub token_set: TokenSetConfig,
    pub database: DatabaseConfig,
}

impl ApiServerConfig {
    pub fn config_schema_pretty_json() -> String {
        serde_json::to_string_pretty(&schema_for!(ApiServerConfigInput))
            .expect("config schema serializes")
    }
}

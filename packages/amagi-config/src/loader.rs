use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::de::DeserializeOwned;

use crate::{
    error::{ConfigError, ConfigResult},
    model::{
        ApiServerConfig, DatabaseConfig, OidcSourceConfig, ServerConfig, TokenSetConfig,
        default_external_base_url,
    },
};

impl ApiServerConfig {
    pub fn load() -> ConfigResult<Self> {
        Self::from_figment(Self::figment()?)
    }

    pub fn figment() -> ConfigResult<Figment> {
        let mut figment = Figment::new();

        for path in config_file_candidates() {
            if path.exists() {
                figment = figment.merge(Toml::file(path));
            }
        }

        Ok(figment.merge(Env::prefixed("AMAGI_").split("__")))
    }

    pub fn from_figment(figment: Figment) -> ConfigResult<Self> {
        let server = extract_or_default::<ServerConfig>(&figment, "server")?;
        let external_base_url = extract_optional::<String>(&figment, "external_base_url")?
            .unwrap_or_else(|| default_external_base_url(&server));
        let config = Self {
            server,
            external_base_url,
            default_oidc_source: extract_optional::<String>(&figment, "default_oidc_source")?,
            oidc_sources: extract_or_default::<BTreeMap<String, OidcSourceConfig>>(
                &figment,
                "oidc_sources",
            )?,
            token_set: extract_or_default::<TokenSetConfig>(&figment, "token_set")?,
            database: extract_or_default::<DatabaseConfig>(&figment, "database")?,
        };

        config.validate()?;
        Ok(config)
    }
}

fn extract_or_default<T>(figment: &Figment, key: &str) -> ConfigResult<T>
where
    T: DeserializeOwned + Default,
{
    if figment.find_value(key).is_err() {
        return Ok(T::default());
    }

    figment
        .extract_inner(key)
        .map_err(|source| ConfigError::Invalid {
            message: format!("failed to load API server config: {source}"),
        })
}

fn extract_optional<T>(figment: &Figment, key: &str) -> ConfigResult<Option<T>>
where
    T: DeserializeOwned,
{
    if figment.find_value(key).is_err() {
        return Ok(None);
    }

    figment
        .extract_inner(key)
        .map(Some)
        .map_err(|source| ConfigError::Invalid {
            message: format!("failed to load API server config: {source}"),
        })
}

fn config_file_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(path) = env::var("AMAGI_CONFIG_FILE") {
        paths.push(PathBuf::from(path));
    }

    for candidate in ["amagi.config.toml", "amagi.toml"] {
        if !paths.iter().any(|path| path == Path::new(candidate)) {
            paths.push(PathBuf::from(candidate));
        }
    }

    paths
}

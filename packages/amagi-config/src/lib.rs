mod error;
mod loader;
mod model;
mod schema;
mod validation;

pub use error::{ConfigError, ConfigResult};
pub use loader::ConfigLoadOptions;
pub use model::{
    AccessTokenSubstrateSourceConfig, ApiServerConfig, BackendOidcFacadePathsConfig,
    BackendOidcSourceConfig, BooleanLike, DEFAULT_OIDC_SOURCE_KEY, DatabaseConfig,
    FrontendOidcSourceConfig, OidcIdentityClaimConfig, OidcSharedSourceConfig, OidcSourceConfig,
    SecretString, ServerConfig, TokenSetConfig, TokenSetStoragePolicyConfig,
};

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        net::SocketAddr,
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use figment::{
        Figment,
        providers::{Env, Format, Serialized, Toml},
    };

    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_vars(pairs: &[(&str, &str)], test: impl FnOnce()) {
        let _guard = env_lock().lock().expect("env lock is not poisoned");

        let previous_values = pairs
            .iter()
            .map(|(key, _)| ((*key).to_owned(), std::env::var(key).ok()))
            .collect::<Vec<_>>();

        for (key, value) in pairs {
            unsafe { std::env::set_var(key, value) };
        }

        test();

        for (key, previous_value) in previous_values {
            match previous_value {
                Some(value) => unsafe { std::env::set_var(&key, value) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }

    #[test]
    fn default_config_has_local_bind_address() {
        let config = ApiServerConfig::default();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 7800);
        assert_eq!(
            config.bind_addr().expect("default bind address is valid"),
            "127.0.0.1:7800"
                .parse::<SocketAddr>()
                .expect("socket address parses")
        );
        assert!(config.oidc_sources.is_empty());
        assert!(config.default_oidc_source.is_none());
    }

    #[test]
    fn explicit_config_file_path_overrides_env_selected_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "amagi-config-loader-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir is created");

        let env_config_path = temp_dir.join("env-config.toml");
        let cli_config_path = temp_dir.join("cli-config.toml");
        fs::write(&env_config_path, "[server]\nport = 7810\n")
            .expect("env config file is written");
        fs::write(&cli_config_path, "[server]\nport = 7820\n")
            .expect("cli config file is written");

        with_env_vars(
            &[(
                "AMAGI_CONFIG_FILE",
                env_config_path.to_str().expect("env config path is valid utf-8"),
            )],
            || {
                let config = ApiServerConfig::load_with_options(ConfigLoadOptions {
                    config_file: Some(cli_config_path.clone()),
                })
                .expect("config loads with explicit path");

                assert_eq!(config.server.port, 7820);
            },
        );

        fs::remove_file(env_config_path).expect("env config file is removed");
        fs::remove_file(cli_config_path).expect("cli config file is removed");
        fs::remove_dir(temp_dir).expect("temp dir is removed");
    }

    #[test]
    fn oidc_client_secret_is_redacted_in_debug_output() {
        let mut oidc_sources = BTreeMap::new();
        oidc_sources.insert(
            DEFAULT_OIDC_SOURCE_KEY.to_owned(),
            OidcSourceConfig {
                oidc: OidcSharedSourceConfig {
                    client_id: Some("amagi".to_owned()),
                    client_secret: Some(SecretString::new("super-secret-value")),
                    ..OidcSharedSourceConfig::default()
                },
                ..OidcSourceConfig::default()
            },
        );

        let config = ApiServerConfig {
            default_oidc_source: Some(DEFAULT_OIDC_SOURCE_KEY.to_owned()),
            oidc_sources,
            ..ApiServerConfig::default()
        };

        let debug_output = format!("{config:?}");
        let oidc = &config.oidc_sources[DEFAULT_OIDC_SOURCE_KEY].oidc;

        assert!(!debug_output.contains("super-secret-value"));
        assert!(debug_output.contains("[REDACTED]"));
        assert_eq!(
            oidc.client_secret
                .as_ref()
                .expect("secret is configured")
                .expose_secret(),
            "super-secret-value"
        );
    }

    #[test]
    fn database_url_is_redacted_in_debug_output() {
        let config = ApiServerConfig {
            database: DatabaseConfig {
                url: Some(SecretString::new(
                    "postgres://amagi:super-secret-password@localhost:5432/amagi",
                )),
                auto_migrate: BooleanLike(false),
            },
            ..ApiServerConfig::default()
        };

        let debug_output = format!("{config:?}");

        assert!(!debug_output.contains("super-secret-password"));
        assert!(!debug_output.contains("postgres://amagi:"));
        assert!(debug_output.contains("[REDACTED]"));
        assert_eq!(
            config
                .database
                .url
                .as_ref()
                .expect("database URL is configured")
                .expose_secret(),
            "postgres://amagi:super-secret-password@localhost:5432/amagi"
        );
        assert!(!bool::from(config.database.auto_migrate));
    }

    #[test]
    fn database_auto_migrate_defaults_to_false() {
        let config = ApiServerConfig::default();

        assert!(!bool::from(config.database.auto_migrate));
    }

    #[test]
    fn figment_config_loads_nested_values() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "server": {
                "host": "0.0.0.0",
                "port": 8900
            },
            "default_oidc_source": "primary",
            "oidc_sources": {
                "primary": {
                    "oidc": {
                        "issuer_url": "https://issuer.example",
                        "client_id": "amagi",
                        "client_secret": "top-secret"
                    },
                    "access_token_substrate": {
                        "audiences": ["api://amagi"]
                    }
                }
            },
            "database": {
                "url": "postgres://amagi:amagi@localhost:5432/amagi",
                "auto_migrate": true
            }
        })));

        let config = ApiServerConfig::from_figment(figment).expect("figment config loads");

        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8900);
        assert_eq!(config.external_base_url, "http://0.0.0.0:8900");
        assert_eq!(config.default_oidc_source.as_deref(), Some("primary"));
        assert_eq!(
            config.oidc_sources["primary"].oidc.client_id.as_deref(),
            Some("amagi")
        );
        assert!(bool::from(config.database.auto_migrate));
    }

    #[test]
    fn multi_oidc_source_merge_overrides_single_source_key() {
        let figment = Figment::new()
            .merge(Serialized::defaults(serde_json::json!({
                "default_oidc_source": "primary",
                "oidc_sources": {
                    "primary": {
                        "oidc": {
                            "issuer_url": "https://issuer.primary",
                            "client_id": "primary-client"
                        }
                    },
                    "secondary": {
                        "oidc": {
                            "issuer_url": "https://issuer.secondary",
                            "client_id": "secondary-client"
                        }
                    }
                }
            })))
            .merge(Serialized::defaults(serde_json::json!({
                "oidc_sources": {
                    "primary": {
                        "backend_oidc": {
                            "client_secret": "override-secret"
                        }
                    }
                }
            })));

        let config = ApiServerConfig::from_figment(figment).expect("config merges");

        assert_eq!(
            config.oidc_sources["primary"]
                .backend_oidc
                .oidc_client
                .client_secret
                .as_ref()
                .expect("override secret is configured")
                .expose_secret(),
            "override-secret"
        );
        assert_eq!(
            config.oidc_sources["secondary"].oidc.client_id.as_deref(),
            Some("secondary-client")
        );
    }

    #[test]
    fn invalid_boolean_like_reports_config_error() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "database": { "auto_migrate": "sometimes" }
        })));

        let error = ApiServerConfig::from_figment(figment).expect_err("invalid bool-like fails");

        assert!(matches!(error, ConfigError::Invalid { .. }));
        assert!(
            error
                .to_string()
                .contains("invalid boolean-like value `sometimes`")
        );
    }

    #[test]
    fn default_oidc_source_must_exist() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "default_oidc_source": "missing"
        })));

        let error = ApiServerConfig::from_figment(figment).expect_err("missing source fails");

        assert!(
            error
                .to_string()
                .contains("default_oidc_source `missing` does not exist")
        );
    }

    #[test]
    fn fixed_callback_paths_are_not_configurable() {
        let config_path =
            std::env::temp_dir().join(format!("amagi-fixed-path-test-{}.toml", std::process::id()));
        fs::write(
            &config_path,
            r#"
    [oidc_sources.primary.oidc]
    redirect_url = "/custom/callback"
"#,
        )
        .expect("temp config can be written");
        let figment = Figment::new().merge(Toml::file(&config_path));

        let error = ApiServerConfig::from_figment(figment)
            .expect_err("explicit fixed callback path is rejected");
        let _ = fs::remove_file(&config_path);

        let message = error.to_string();
        assert!(message.contains("redirect_url"), "{message}");
    }

    #[test]
    fn token_propagation_enablement_is_rejected() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "oidc_sources": {
                "primary": {
                    "access_token_substrate": {
                        "token_propagation": {
                            "kind": "enabled"
                        }
                    }
                }
            }
        })));

        let error =
            ApiServerConfig::from_figment(figment).expect_err("token propagation enabling fails");

        assert!(error.to_string().contains("token_propagation is disabled"));
    }

    #[test]
    fn unsafe_frontend_client_secret_is_rejected() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "oidc_sources": {
                "primary": {
                    "frontend_oidc": {
                        "unsafe_frontend_client_secret": "enabled"
                    }
                }
            }
        })));

        let error = ApiServerConfig::from_figment(figment)
            .expect_err("unsafe frontend client secret enabling fails");

        assert!(
            error
                .to_string()
                .contains("unsafe_frontend_client_secret is disabled")
        );
    }

    #[test]
    fn oidc_identity_claim_defaults_to_sub() {
        let config = ApiServerConfig::default();

        assert!(config.oidc_sources.is_empty());
        assert_eq!(
            OidcSourceConfig::default().oidc_identity_claim,
            OidcIdentityClaimConfig::Sub
        );
    }

    #[test]
    fn custom_claim_identity_claim_parses_from_figment() {
        let figment = Figment::new().merge(Serialized::defaults(serde_json::json!({
            "default_oidc_source": "primary",
            "oidc_sources": {
                "primary": {
                    "oidc_identity_claim": {
                        "type": "custom_claim",
                        "claim_name": "employee_id"
                    }
                }
            }
        })));

        let config = ApiServerConfig::from_figment(figment).expect("config parses");

        assert_eq!(
            config.oidc_sources["primary"].oidc_identity_claim,
            OidcIdentityClaimConfig::CustomClaim {
                claim_name: "employee_id".to_owned()
            }
        );
    }

    #[test]
    fn nested_env_keys_are_loaded_from_formal_overlay() {
        with_env_vars(
            &[
                ("AMAGI_SERVER__HOST", "0.0.0.0"),
                ("AMAGI_SERVER__PORT", "8901"),
                (
                    "AMAGI_DATABASE__URL",
                    "postgres://amagi:amagi@localhost:5432/amagi",
                ),
                ("AMAGI_DATABASE__AUTO_MIGRATE", "yes"),
                ("AMAGI_DEFAULT_OIDC_SOURCE", "default"),
                (
                    "AMAGI_OIDC_SOURCES__default__OIDC__ISSUER_URL",
                    "https://issuer.example",
                ),
                ("AMAGI_OIDC_SOURCES__default__OIDC__CLIENT_ID", "amagi"),
                (
                    "AMAGI_OIDC_SOURCES__default__OIDC__CLIENT_SECRET",
                    "top-secret",
                ),
            ],
            || {
                let figment = Figment::new().merge(Env::prefixed("AMAGI_").split("__"));
                let config = ApiServerConfig::from_figment(figment)
                    .expect("formal nested env overlay parses");

                assert_eq!(config.server.host, "0.0.0.0");
                assert_eq!(config.server.port, 8901);
                assert_eq!(config.default_oidc_source.as_deref(), Some("default"));
                assert_eq!(
                    config.oidc_sources[DEFAULT_OIDC_SOURCE_KEY]
                        .oidc
                        .remote
                        .issuer_url
                        .as_deref(),
                    Some("https://issuer.example")
                );
                assert_eq!(
                    config.oidc_sources[DEFAULT_OIDC_SOURCE_KEY]
                        .oidc
                        .client_id
                        .as_deref(),
                    Some("amagi")
                );
                assert_eq!(
                    config
                        .database
                        .url
                        .as_ref()
                        .expect("database url is configured")
                        .expose_secret(),
                    "postgres://amagi:amagi@localhost:5432/amagi"
                );
                assert!(bool::from(config.database.auto_migrate));
            },
        );
    }

    #[test]
    fn nested_env_overlay_merges_map_like_oidc_sources() {
        with_env_vars(
            &[
                ("AMAGI_DEFAULT_OIDC_SOURCE", "default"),
                (
                    "AMAGI_OIDC_SOURCES__default__OIDC__CLIENT_ID",
                    "default-client",
                ),
                (
                    "AMAGI_OIDC_SOURCES__default__OIDC__ISSUER_URL",
                    "https://issuer.default",
                ),
                (
                    "AMAGI_OIDC_SOURCES__workforce__OIDC__CLIENT_ID",
                    "workforce-client",
                ),
                (
                    "AMAGI_OIDC_SOURCES__workforce__OIDC__ISSUER_URL",
                    "https://issuer.workforce",
                ),
            ],
            || {
                let figment = Figment::new().merge(Env::prefixed("AMAGI_").split("__"));
                let config = ApiServerConfig::from_figment(figment)
                    .expect("formal nested env map overlay parses");

                assert_eq!(config.default_oidc_source.as_deref(), Some("default"));
                assert_eq!(config.oidc_sources.len(), 2);
                assert_eq!(
                    config.oidc_sources[DEFAULT_OIDC_SOURCE_KEY]
                        .oidc
                        .client_id
                        .as_deref(),
                    Some("default-client")
                );
                assert_eq!(
                    config.oidc_sources["workforce"]
                        .oidc
                        .remote
                        .issuer_url
                        .as_deref(),
                    Some("https://issuer.workforce")
                );
            },
        );
    }

    #[test]
    fn example_config_file_parses() {
        let example_path = workspace_root().join("amagi.config.example.toml");
        let figment = Figment::new().merge(Toml::file(&example_path));

        let config = ApiServerConfig::from_figment(figment).expect("example config parses");

        assert_eq!(config.default_oidc_source.as_deref(), Some("default"));
        assert!(config.oidc_sources.contains_key("default"));
        assert!(config.oidc_sources.contains_key("workforce"));
        assert_eq!(
            config.oidc_sources["default"]
                .access_token_substrate
                .resource_server
                .audiences,
            vec!["api://amagi"]
        );
    }

    #[test]
    fn committed_config_schema_matches_generated_snapshot() {
        let schema_path = workspace_root().join("amagi.config.schema.json");
        let committed_schema =
            fs::read_to_string(&schema_path).expect("committed config schema exists");
        let committed_schema_json: serde_json::Value = serde_json::from_str(&committed_schema)
            .expect("committed config schema parses as JSON");
        let generated_schema_json: serde_json::Value =
            serde_json::from_str(&ApiServerConfig::config_schema_pretty_json())
                .expect("generated config schema parses as JSON");

        assert_eq!(committed_schema_json, generated_schema_json);
    }
}

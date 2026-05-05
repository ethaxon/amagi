use amagi_config::{ApiServerConfig, OidcSharedSourceConfig, OidcSourceConfig, SecretString};
use amagi_securitydept::{
    AuthRuntime, BackendOidcModeAuthorizeQuery, BackendOidcModeUserInfoRequest,
    OidcCodeCallbackSearchParams,
};
use amagi_test_utils::dex::{
    auth_browser_client, complete_dex_authorization_code_flow, start_dex_with_private_client,
};
use serde_json::Value;

fn sample_config(well_known_url: String) -> ApiServerConfig {
    let mut config = ApiServerConfig {
        external_base_url: "http://127.0.0.1:7800".to_owned(),
        default_oidc_source: Some("primary".to_owned()),
        ..ApiServerConfig::default()
    };

    config.oidc_sources.insert(
        "primary".to_owned(),
        OidcSourceConfig {
            oidc: OidcSharedSourceConfig {
                remote: securitydept_core::oauth_provider::OAuthProviderRemoteConfig {
                    well_known_url: Some(well_known_url),
                    ..Default::default()
                },
                client_id: Some("client".to_owned()),
                client_secret: Some(SecretString::new("secret")),
                scopes: vec![
                    "openid".to_owned(),
                    "profile".to_owned(),
                    "email".to_owned(),
                ],
                ..OidcSharedSourceConfig::default()
            },
            ..OidcSourceConfig::default()
        },
    );

    config
}

#[tokio::test]
async fn dex_backed_start_and_user_info_work_against_real_provider() {
    let redirect_uri = "http://127.0.0.1:7800/api/auth/token-set/oidc/source/primary/callback";
    let dex =
        start_dex_with_private_client(redirect_uri, "client", "Amagi Test Client", "secret", true)
            .await;
    let issuer_base = dex.issuer_base().to_owned();
    let config = sample_config(format!("{issuer_base}/.well-known/openid-configuration"));
    let runtime = AuthRuntime::from_api_config(&config);

    let start_response = runtime
        .oidc_start("primary", &BackendOidcModeAuthorizeQuery::default())
        .await
        .expect("oidc start succeeds against dex");
    assert!(matches!(start_response.status, 302 | 303 | 307));
    let location = start_response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("location"))
        .map(|(_, value)| value.as_str())
        .expect("redirect location header is present");
    assert!(location.starts_with(&format!("{issuer_base}/auth")));
    assert!(location.contains("client_id=client"));

    let token_response: Value = reqwest::Client::new()
        .post(format!("{issuer_base}/token"))
        .header("Authorization", "Basic Y2xpZW50OnNlY3JldA==")
        .form(&[
            ("grant_type", "password"),
            ("scope", "openid profile email"),
            ("username", "user@example.org"),
            ("password", "user"),
        ])
        .send()
        .await
        .expect("dex token request succeeds")
        .error_for_status()
        .expect("dex token request returns success")
        .json()
        .await
        .expect("dex token response is json");

    let access_token = token_response["access_token"]
        .as_str()
        .expect("access token is present");
    let id_token = token_response["id_token"]
        .as_str()
        .expect("id token is present");

    let user_info = runtime
        .oidc_user_info(
            "primary",
            &BackendOidcModeUserInfoRequest {
                id_token: id_token.to_owned(),
            },
            access_token,
        )
        .await
        .expect("user info succeeds against dex");

    assert!(!user_info.subject.is_empty());
    let claims = user_info.claims.as_ref().expect("claims are present");
    assert_eq!(
        claims.get("email").and_then(|value| value.as_str()),
        Some("user@example.org")
    );
    let claim_keys = claims.keys().cloned().collect::<Vec<_>>();
    assert!(!claim_keys.iter().any(|key| matches!(
        key.as_str(),
        "access_token" | "refresh_token" | "id_token" | "client_secret" | "authorization"
    )));
}

#[tokio::test]
async fn dex_backed_callback_body_return_exchanges_real_authorization_code() {
    let redirect_uri = "http://127.0.0.1:7800/api/auth/token-set/oidc/source/primary/callback";
    let dex =
        start_dex_with_private_client(redirect_uri, "client", "Amagi Test Client", "secret", false)
            .await;
    let issuer_base = dex.issuer_base().to_owned();
    let config = sample_config(format!("{issuer_base}/.well-known/openid-configuration"));
    let runtime = AuthRuntime::from_api_config(&config);

    let start_response = runtime
        .oidc_start("primary", &BackendOidcModeAuthorizeQuery::default())
        .await
        .expect("oidc start succeeds against dex");
    let start_url = start_response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("location"))
        .map(|(_, value)| value.as_str())
        .expect("redirect location header is present");

    let callback_url =
        complete_dex_authorization_code_flow(&auth_browser_client(), start_url, redirect_uri).await;
    let code = callback_url
        .query_pairs()
        .find_map(|(key, value)| (key == "code").then(|| value.into_owned()))
        .expect("callback code is present");
    let state = callback_url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("callback state is present");

    let callback_body = runtime
        .oidc_callback_body_return(
            "primary",
            OidcCodeCallbackSearchParams {
                code,
                state: Some(state.clone()),
            },
        )
        .await
        .expect("callback body return succeeds against dex auth code");

    assert!(
        callback_body
            .get("access_token")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        callback_body
            .get("id_token")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.is_empty())
    );
    assert!(callback_body.get("metadata").is_some());
    assert!(callback_body.get("metadata_redemption_id").is_none());
    assert!(callback_body.get("client_secret").is_none());
    assert!(callback_body.get("authorization").is_none());
    assert!(callback_body.get("code").is_none());
    assert!(callback_body.get("state").is_none());
}

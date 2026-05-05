use std::collections::BTreeMap;

use amagi_auth::AuthError;
use amagi_securitydept::{
    BackendOidcModeAuthorizeQuery, BackendOidcModeMetadataRedemptionRequest,
    BackendOidcModeMetadataRedemptionResponse, BackendOidcModeRefreshPayload,
    BackendOidcModeUserInfoRequest, OidcCodeCallbackSearchParams, SecurityDeptHttpResponse,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;

use crate::app::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/auth/token-set/oidc/source/{source}/start",
            get(oidc_start),
        )
        .route(
            "/api/auth/token-set/oidc/source/{source}/config",
            get(frontend_config_projection),
        )
        .route(
            "/api/auth/token-set/oidc/source/{source}/callback",
            get(backend_callback_fragment).post(backend_callback_body),
        )
        .route(
            "/api/auth/token-set/oidc/source/{source}/refresh",
            post(oidc_refresh),
        )
        .route(
            "/api/auth/token-set/oidc/source/{source}/metadata/redeem",
            post(oidc_metadata_redeem),
        )
        .route(
            "/api/auth/token-set/oidc/source/{source}/user-info",
            post(oidc_user_info),
        )
        .route(
            "/auth/token-set/oidc/source/{source}/callback",
            get(frontend_callback_shell),
        )
}

async fn frontend_config_projection(
    Path(source): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<amagi_auth::FrontendOidcConfigProjection>, AuthApiError> {
    state
        .auth_facade
        .frontend_config_projection(&source)
        .await
        .map(Json)
        .map_err(AuthApiError)
}

async fn oidc_start(
    Path(source): Path<String>,
    Query(query): Query<BackendOidcModeAuthorizeQuery>,
    State(state): State<AppState>,
) -> Result<Response, AuthApiError> {
    let response = state
        .auth_facade
        .oidc_start(&source, &query)
        .await
        .map_err(AuthApiError)?;
    Ok(into_securitydept_response(response))
}

async fn backend_callback_fragment(
    Path(source): Path<String>,
    Query(query): Query<OidcCodeCallbackSearchParams>,
    State(state): State<AppState>,
) -> Result<Response, AuthApiError> {
    let response = state
        .auth_facade
        .oidc_callback_fragment_return(&source, &query)
        .await
        .map_err(AuthApiError)?;
    Ok(into_securitydept_response(response))
}

async fn backend_callback_body(
    Path(source): Path<String>,
    Query(query): Query<OidcCodeCallbackSearchParams>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AuthApiError> {
    state
        .auth_facade
        .oidc_callback_body_return(&source, &query)
        .await
        .map(Json)
        .map_err(AuthApiError)
}

async fn oidc_refresh(
    Path(source): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<BackendOidcModeRefreshPayload>,
) -> Result<Json<serde_json::Value>, AuthApiError> {
    state
        .auth_facade
        .oidc_refresh_body_return(&source, &payload)
        .await
        .map(Json)
        .map_err(AuthApiError)
}

async fn oidc_metadata_redeem(
    Path(source): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<BackendOidcModeMetadataRedemptionRequest>,
) -> Result<Json<BackendOidcModeMetadataRedemptionResponse>, AuthApiError> {
    state
        .auth_facade
        .oidc_metadata_redeem(&source, &payload)
        .await
        .map(Json)
        .map_err(AuthApiError)
}

async fn oidc_user_info(
    Path(source): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BackendOidcModeUserInfoRequest>,
) -> Result<Json<amagi_auth::AuthenticatedOidcUserInfoResponse>, AuthApiError> {
    state
        .auth_facade
        .oidc_user_info(&source, &payload, extract_bearer_token(&headers))
        .await
        .map(Json)
        .map_err(AuthApiError)
}

async fn frontend_callback_shell(
    Path(source): Path<String>,
    Query(query): Query<BTreeMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<amagi_auth::FrontendCallbackShellResponse>, AuthApiError> {
    state
        .auth_facade
        .frontend_callback_shell(&source, &query)
        .map(Json)
        .map_err(AuthApiError)
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.trim().is_empty())
}

fn into_securitydept_response(response: SecurityDeptHttpResponse) -> Response {
    let mut axum_response = Response::new(Body::empty());
    *axum_response.status_mut() =
        StatusCode::from_u16(response.status).expect("securitydept status must be valid");

    for (name, value) in response.headers {
        let Ok(name) = HeaderName::try_from(name) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            continue;
        };
        axum_response.headers_mut().insert(name, value);
    }

    axum_response
}

#[derive(Debug)]
struct AuthApiError(AuthError);

#[derive(Debug, Serialize)]
struct AuthErrorResponse {
    code: &'static str,
    message: String,
    source: Option<String>,
}

impl IntoResponse for AuthApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.http_status_code())
            .expect("auth error status code is valid");
        let payload = AuthErrorResponse {
            code: self.0.code(),
            message: self.0.to_string(),
            source: self.0.source_key().map(ToOwned::to_owned),
        };

        (status, Json(payload)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use amagi_config::ApiServerConfig;
    use axum::{body::to_bytes, http::Request};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::app::{build_app, build_state};

    fn sample_config() -> ApiServerConfig {
        let mut config = ApiServerConfig {
            default_oidc_source: Some("primary".to_owned()),
            ..ApiServerConfig::default()
        };

        config.oidc_sources.insert(
            "primary".to_owned(),
            serde_json::from_value(serde_json::json!({
                "oidc": {
                    "issuer_url": "https://issuer.primary",
                    "well_known_url": "https://issuer.primary/.well-known/openid-configuration",
                    "client_id": "interactive-client",
                    "client_secret": "backend-secret"
                }
            }))
            .expect("oidc source config parses"),
        );

        config
    }

    #[tokio::test]
    async fn frontend_config_projection_hides_client_secret() {
        let app = build_app(build_state(sample_config()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/token-set/oidc/source/primary/config")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let payload: Value = serde_json::from_slice(&body).expect("response is JSON");

        assert_eq!(payload["source"], "primary");
        assert_eq!(
            payload["configProjectionPath"],
            "/api/auth/token-set/oidc/source/primary/config"
        );
        assert_eq!(payload["clientId"], "interactive-client");
        assert_eq!(
            payload["redirectPath"],
            "/auth/token-set/oidc/source/primary/callback"
        );
        assert!(payload.get("client_secret").is_none());
        assert!(payload.get("clientSecret").is_none());
    }

    #[tokio::test]
    async fn frontend_callback_shell_sanitizes_query_values() {
        let app = build_app(build_state(sample_config()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(
                        "/auth/token-set/oidc/source/primary/callback?code=secret-code&\
                         state=opaque-state&refresh_token=must-not-appear",
                    )
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let body_text = String::from_utf8(body.to_vec()).expect("body is UTF-8");
        let payload: Value = serde_json::from_str(&body_text).expect("response is JSON");

        assert_eq!(payload["source"], "primary");
        assert_eq!(payload["query"]["code_present"], true);
        assert_eq!(payload["query"]["state_present"], true);
        assert!(!body_text.contains("secret-code"));
        assert!(!body_text.contains("opaque-state"));
        assert!(!body_text.contains("must-not-appear"));
    }

    #[tokio::test]
    async fn unknown_oidc_source_returns_structured_not_found() {
        let app = build_app(build_state(sample_config()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/token-set/oidc/source/missing/start")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let payload: Value = serde_json::from_slice(&body).expect("response is JSON");

        assert_eq!(payload["code"], "auth.oidc.source_unknown");
        assert_eq!(payload["source"], "missing");
    }

    #[tokio::test]
    async fn user_info_requires_bearer_token_before_runtime_call() {
        let app = build_app(build_state(sample_config()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/token-set/oidc/source/primary/user-info")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"id_token":"opaque-id-token"}"#))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let payload: Value = serde_json::from_slice(&body).expect("response is JSON");

        assert_eq!(payload["code"], "auth_access_token_missing");
    }
}

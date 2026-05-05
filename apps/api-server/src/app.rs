use std::sync::Arc;

use amagi_auth::AuthFacadeService;
use amagi_bookmarks::BookmarkService;
use amagi_config::ApiServerConfig;
use amagi_db::DatabaseService;
use amagi_securitydept::AuthRuntime;
use amagi_sync::SyncService;
use axum::Router;
use tower_http::cors::CorsLayer;
#[cfg(test)]
use uuid::Uuid;

use crate::http::routes;

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<ApiServerConfig>,
    pub auth: AuthRuntime,
    pub auth_facade: AuthFacadeService,
    pub bookmarks: BookmarkService,
    pub database: DatabaseService,
    pub sync: SyncService,
    #[cfg(test)]
    pub dashboard_principal_override: Option<DashboardPrincipalOverride>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub enum DashboardPrincipalOverride {
    Bound { user_id: Uuid },
    Unbound,
}

pub async fn build_state(config: ApiServerConfig) -> AppState {
    let auth = AuthRuntime::from_api_config(&config);
    let database = DatabaseService::initialize(&config.database).await;
    let config = Arc::new(config);
    let auth_facade = AuthFacadeService::new(Arc::clone(&config), auth.clone(), database.clone());
    let bookmarks = BookmarkService::new(database.clone());
    let sync = SyncService::new(database.clone(), bookmarks.clone());

    AppState {
        auth,
        auth_facade,
        bookmarks,
        config,
        database,
        sync,
        #[cfg(test)]
        dashboard_principal_override: None,
    }
}

pub fn build_app(state: AppState) -> Router {
    routes::router()
        .layer(dashboard_dev_cors_layer())
        .with_state(state)
}

fn dashboard_dev_cors_layer() -> CorsLayer {
    use axum::http::{HeaderValue, Method, header};

    CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:4174"),
            HeaderValue::from_static("http://127.0.0.1:4174"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-amagi-oidc-source"),
        ])
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_endpoint_reports_service_status() {
        let app = build_app(build_state(ApiServerConfig::default()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let payload: Value = serde_json::from_slice(&body).expect("health response is JSON");

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["service"], "amagi-api-server");
        assert_eq!(payload["database"]["state"], "not_configured");
    }

    #[tokio::test]
    async fn readiness_endpoint_reports_not_ready_without_database() {
        let app = build_app(build_state(ApiServerConfig::default()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body is readable");
        let payload: Value = serde_json::from_slice(&body).expect("readiness response is JSON");

        assert_eq!(payload["status"], "not_ready");
        assert_eq!(payload["database"]["state"], "not_configured");
    }

    #[tokio::test]
    async fn dashboard_dev_origin_preflight_is_allowed() {
        let app = build_app(build_state(ApiServerConfig::default()).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/dashboard/sync-profiles")
                    .header(header::ORIGIN, "http://localhost:4174")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .header(
                        header::ACCESS_CONTROL_REQUEST_HEADERS,
                        "authorization,x-amagi-oidc-source,content-type",
                    )
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|value| value.to_str().ok()),
            Some("http://localhost:4174")
        );
        let allowed_headers = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .and_then(|value| value.to_str().ok())
            .expect("allow headers present")
            .to_ascii_lowercase();
        assert!(allowed_headers.contains("authorization"));
        assert!(allowed_headers.contains("content-type"));
        assert!(allowed_headers.contains("x-amagi-oidc-source"));
    }
}

use std::sync::Arc;

use amagi_auth::AuthFacadeService;
use amagi_bookmarks::BookmarkService;
use amagi_config::ApiServerConfig;
use amagi_db::DatabaseService;
use amagi_securitydept::AuthRuntime;
use amagi_sync::SyncService;
use axum::Router;
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
    routes::router().with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
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
}

use amagi_db::DatabaseState;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use crate::app::AppState;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    database: amagi_db::DatabaseReport,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: &'static str,
    service: &'static str,
    database: amagi_db::DatabaseReport,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
}

async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    let database = state.database.health_report();
    let _external_base_url = state.config.external_base_url.as_str();
    let _auth_start_path = state
        .auth
        .securitydept
        .token_set
        .facade_paths
        .start
        .as_str();
    let _securitydept_surface = state
        .auth
        .securitydept
        .token_set
        .securitydept_backend_oidc_surface;

    Json(HealthResponse {
        status: "ok",
        service: "amagi-api-server",
        database,
    })
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    let database = state.database.readiness_report().await;
    let ready = matches!(database.state, DatabaseState::Ready);

    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(ReadinessResponse {
            status: if ready { "ok" } else { "not_ready" },
            service: "amagi-api-server",
            database,
        }),
    )
}

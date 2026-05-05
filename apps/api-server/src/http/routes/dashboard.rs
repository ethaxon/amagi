use amagi_auth::AuthError;
use amagi_bookmarks::{
    BookmarkError, BookmarkNodeView, CreateLibraryRequest, CreateNodeRequest, LibraryTreeView,
    LibraryView, MoveNodeRequest, RestoreNodeRequest, RevisionFeedView, UpdateNodeRequest,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;

const SOURCE_HEADER: &str = "x-amagi-oidc-source";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/dashboard/me", get(me))
        .route(
            "/api/v1/dashboard/libraries",
            get(list_libraries).post(create_library),
        )
        .route(
            "/api/v1/dashboard/libraries/{library_id}/tree",
            get(library_tree),
        )
        .route(
            "/api/v1/dashboard/libraries/{library_id}/nodes",
            post(create_node),
        )
        .route("/api/v1/dashboard/nodes/{node_id}", patch(update_node))
        .route("/api/v1/dashboard/nodes/{node_id}/move", post(move_node))
        .route(
            "/api/v1/dashboard/nodes/{node_id}/delete",
            post(delete_node),
        )
        .route(
            "/api/v1/dashboard/nodes/{node_id}/restore",
            post(restore_node),
        )
        .route(
            "/api/v1/dashboard/libraries/{library_id}/revisions",
            get(revision_feed),
        )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardMeView {
    user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevisionFeedQuery {
    after_clock: Option<i64>,
    limit: Option<u64>,
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardMeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    Ok(Json(DashboardMeView {
        user_id: user_id.to_string(),
    }))
}

async fn list_libraries(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LibraryView>>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .list_libraries(user_id)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn create_library(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<LibraryTreeView>), DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .create_library(user_id, &payload)
        .await
        .map(|view| (StatusCode::CREATED, Json(view)))
        .map_err(DashboardApiError::from)
}

async fn library_tree(
    Path(library_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LibraryTreeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .tree(user_id, parse_uuid(&library_id, "library_not_found")?)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn create_node(
    Path(library_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNodeRequest>,
) -> Result<(StatusCode, Json<BookmarkNodeView>), DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .create_node(
            user_id,
            parse_uuid(&library_id, "library_not_found")?,
            &payload,
        )
        .await
        .map(|view| (StatusCode::CREATED, Json(view)))
        .map_err(DashboardApiError::from)
}

async fn update_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateNodeRequest>,
) -> Result<Json<BookmarkNodeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .update_node(user_id, parse_uuid(&node_id, "node_not_found")?, &payload)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn move_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<MoveNodeRequest>,
) -> Result<Json<BookmarkNodeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .move_node(user_id, parse_uuid(&node_id, "node_not_found")?, &payload)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn delete_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BookmarkNodeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .delete_node(user_id, parse_uuid(&node_id, "node_not_found")?)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn restore_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RestoreNodeRequest>,
) -> Result<Json<BookmarkNodeView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .restore_node(user_id, parse_uuid(&node_id, "node_not_found")?, &payload)
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn revision_feed(
    Path(library_id): Path<String>,
    Query(query): Query<RevisionFeedQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RevisionFeedView>, DashboardApiError> {
    let user_id = resolve_dashboard_user_id(&state, &headers).await?;
    state
        .bookmarks
        .revisions(
            user_id,
            parse_uuid(&library_id, "library_not_found")?,
            query.after_clock.unwrap_or_default(),
            query.limit.unwrap_or(100),
        )
        .await
        .map(Json)
        .map_err(DashboardApiError::from)
}

async fn resolve_dashboard_user_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, DashboardApiError> {
    let authorization_header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("Bearer "))
        .ok_or(DashboardApiError::Unauthenticated {
            message: "dashboard API requires Authorization: Bearer credentials".to_owned(),
        })?;

    let source = dashboard_source(state, headers)?;

    #[cfg(test)]
    if let Some(override_result) = state.dashboard_principal_override {
        return match override_result {
            crate::app::DashboardPrincipalOverride::Bound { user_id } => Ok(user_id),
            crate::app::DashboardPrincipalOverride::Unbound => {
                Err(DashboardApiError::Unauthenticated {
                    message: "bearer token is valid but no amagi account binding exists".to_owned(),
                })
            }
        };
    }

    let resolution = state
        .auth_facade
        .authenticate_bearer_principal(&source, Some(authorization_header))
        .await
        .map_err(DashboardApiError::from)?;
    let principal = resolution
        .and_then(|resolution| resolution.principal)
        .ok_or(DashboardApiError::Unauthenticated {
            message: "bearer token is valid but no amagi account binding exists".to_owned(),
        })?;

    Uuid::parse_str(&principal.user_id).map_err(|_| DashboardApiError::Forbidden {
        message: "resolved principal has an invalid user id".to_owned(),
    })
}

fn dashboard_source(state: &AppState, headers: &HeaderMap) -> Result<String, DashboardApiError> {
    if let Some(source) = headers
        .get(SOURCE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(source.to_owned());
    }

    state
        .config
        .default_oidc_source
        .clone()
        .ok_or(DashboardApiError::BadRequest {
            code: "oidc_source_required",
            message: "dashboard API requires X-Amagi-Oidc-Source or default_oidc_source".to_owned(),
        })
}

fn parse_uuid(value: &str, not_found_code: &'static str) -> Result<Uuid, DashboardApiError> {
    Uuid::parse_str(value).map_err(|_| DashboardApiError::BadRequest {
        code: not_found_code,
        message: "path id must be a UUID".to_owned(),
    })
}

#[derive(Debug)]
enum DashboardApiError {
    Bookmark(BookmarkError),
    Auth(AuthError),
    Unauthenticated { message: String },
    Forbidden { message: String },
    BadRequest { code: &'static str, message: String },
}

#[derive(Debug, Serialize)]
struct DashboardErrorResponse {
    code: &'static str,
    message: String,
    source: Option<String>,
}

impl From<BookmarkError> for DashboardApiError {
    fn from(value: BookmarkError) -> Self {
        Self::Bookmark(value)
    }
}

impl From<AuthError> for DashboardApiError {
    fn from(value: AuthError) -> Self {
        Self::Auth(value)
    }
}

impl IntoResponse for DashboardApiError {
    fn into_response(self) -> Response {
        let (status, code, message, source) = match self {
            Self::Bookmark(error) => (
                StatusCode::from_u16(error.http_status_code()).expect("status code is valid"),
                error.code(),
                error.to_string(),
                None,
            ),
            Self::Auth(error) => (
                StatusCode::from_u16(error.http_status_code()).expect("status code is valid"),
                error.code(),
                error.to_string(),
                error.source_key().map(ToOwned::to_owned),
            ),
            Self::Unauthenticated { message } => {
                (StatusCode::UNAUTHORIZED, "unauthenticated", message, None)
            }
            Self::Forbidden { message } => (StatusCode::FORBIDDEN, "forbidden", message, None),
            Self::BadRequest { code, message } => (StatusCode::BAD_REQUEST, code, message, None),
        };

        (
            status,
            Json(DashboardErrorResponse {
                code,
                message,
                source,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use amagi_config::ApiServerConfig;
    use amagi_db::{CurrentUserId, entities::users, set_current_user_id};
    use amagi_test_utils::postgres::start_amagi_postgres;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::app::{DashboardPrincipalOverride, build_app, build_state};

    #[tokio::test]
    async fn dashboard_libraries_require_bearer_token() {
        let app = build_app(build_state(sample_config(None)).await);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/dashboard/libraries")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn dashboard_returns_unauthorized_when_bearer_has_no_binding() {
        let mut state = build_state(sample_config(None)).await;
        state.dashboard_principal_override = Some(DashboardPrincipalOverride::Unbound);
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/dashboard/libraries")
                    .header("authorization", "Bearer test-token")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn bound_dashboard_principal_can_create_library_and_read_tree() {
        let postgres = start_amagi_postgres().await;
        let user_id = Uuid::now_v7();
        let mut state = build_state(sample_config(Some(postgres.database_url()))).await;
        insert_user(&state, user_id).await;
        state.dashboard_principal_override = Some(DashboardPrincipalOverride::Bound { user_id });
        let app = build_app(state);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/dashboard/libraries")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Default","kind":"normal"}"#))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let create_payload = response_json(create_response).await;
        assert_eq!(create_payload["library"]["name"], "Default");
        assert_eq!(create_payload["nodes"][0]["nodeType"], "folder");
        assert!(create_payload.to_string().find("test-token").is_none());
        assert!(create_payload.to_string().find("Authorization").is_none());

        let library_id = create_payload["library"]["id"]
            .as_str()
            .expect("library id");
        let tree_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/dashboard/libraries/{library_id}/tree"))
                    .header("authorization", "Bearer test-token")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(tree_response.status(), StatusCode::OK);
        let tree_payload = response_json(tree_response).await;
        assert_eq!(tree_payload["library"]["currentRevisionClock"], 1);
        assert_eq!(
            tree_payload["nodes"].as_array().expect("nodes array").len(),
            1
        );
    }

    fn sample_config(database_url: Option<&str>) -> ApiServerConfig {
        let mut config = ApiServerConfig {
            default_oidc_source: Some("primary".to_owned()),
            ..ApiServerConfig::default()
        };
        if let Some(database_url) = database_url {
            config.database = serde_json::from_value(json!({
                "url": database_url,
                "auto_migrate": true,
            }))
            .expect("database config parses");
        }
        config
    }

    async fn insert_user(state: &AppState, user_id: Uuid) {
        let txn = state
            .database
            .runtime()
            .expect("database runtime is available")
            .connection()
            .begin()
            .await
            .expect("transaction starts");
        set_current_user_id(&txn, CurrentUserId::new(user_id))
            .await
            .expect("current user sets");
        users::ActiveModel {
            id: Set(user_id),
            status: Set("active".to_owned()),
            ..Default::default()
        }
        .insert(&txn)
        .await
        .expect("user inserts");
        txn.commit().await.expect("transaction commits");
    }

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body reads");
        serde_json::from_slice(&body).expect("response body is json")
    }
}

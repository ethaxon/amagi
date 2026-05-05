use amagi_auth::AuthError;
use amagi_sync::{
    CursorAckRequest, CursorAckResponse, FeedRequest, FeedResponse, RegisterClientRequest,
    RegisterClientResponse, SyncApplyRequest, SyncApplyResponse, SyncError, SyncPreviewRequest,
    SyncPreviewResponse, SyncSessionStartRequest, SyncSessionStartResponse,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use uuid::Uuid;

use crate::app::AppState;

const SOURCE_HEADER: &str = "x-amagi-oidc-source";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sync/clients/register", post(register_client))
        .route("/api/v1/sync/session/start", post(start_session))
        .route("/api/v1/sync/feed", get(feed))
        .route("/api/v1/sync/preview", post(preview))
        .route("/api/v1/sync/apply", post(apply))
        .route("/api/v1/sync/cursors/ack", post(ack_cursor))
}

async fn register_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterClientRequest>,
) -> Result<(StatusCode, Json<RegisterClientResponse>), SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .register_client(user_id, &payload)
        .await
        .map(|view| (StatusCode::CREATED, Json(view)))
        .map_err(SyncApiError::from)
}

async fn start_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncSessionStartRequest>,
) -> Result<Json<SyncSessionStartResponse>, SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .start_session(user_id, &payload)
        .await
        .map(Json)
        .map_err(SyncApiError::from)
}

async fn feed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<FeedRequest>,
) -> Result<Json<FeedResponse>, SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .feed(user_id, &query)
        .await
        .map(Json)
        .map_err(SyncApiError::from)
}

async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncPreviewRequest>,
) -> Result<Json<SyncPreviewResponse>, SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .preview(user_id, &payload)
        .await
        .map(Json)
        .map_err(SyncApiError::from)
}

async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncApplyRequest>,
) -> Result<Json<SyncApplyResponse>, SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .apply(user_id, &payload)
        .await
        .map(Json)
        .map_err(SyncApiError::from)
}

async fn ack_cursor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CursorAckRequest>,
) -> Result<Json<CursorAckResponse>, SyncApiError> {
    let user_id = resolve_sync_user_id(&state, &headers).await?;
    state
        .sync
        .ack_cursor(user_id, &payload)
        .await
        .map(Json)
        .map_err(SyncApiError::from)
}

async fn resolve_sync_user_id(state: &AppState, headers: &HeaderMap) -> Result<Uuid, SyncApiError> {
    let authorization_header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.starts_with("Bearer "))
        .ok_or(SyncApiError::Unauthenticated {
            message: "sync API requires Authorization: Bearer credentials".to_owned(),
        })?;

    let source = sync_source(state, headers)?;

    #[cfg(test)]
    if let Some(override_result) = state.dashboard_principal_override {
        return match override_result {
            crate::app::DashboardPrincipalOverride::Bound { user_id } => Ok(user_id),
            crate::app::DashboardPrincipalOverride::Unbound => Err(SyncApiError::Unauthenticated {
                message: "bearer token is valid but no amagi account binding exists".to_owned(),
            }),
        };
    }

    let resolution = state
        .auth_facade
        .authenticate_bearer_principal(&source, Some(authorization_header))
        .await
        .map_err(SyncApiError::from)?;
    let principal = resolution
        .and_then(|resolution| resolution.principal)
        .ok_or(SyncApiError::Unauthenticated {
            message: "bearer token is valid but no amagi account binding exists".to_owned(),
        })?;

    Uuid::parse_str(&principal.user_id).map_err(|_| SyncApiError::Forbidden {
        message: "resolved principal has an invalid user id".to_owned(),
    })
}

fn sync_source(state: &AppState, headers: &HeaderMap) -> Result<String, SyncApiError> {
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
        .ok_or(SyncApiError::BadRequest {
            code: "oidc_source_required",
            message: "sync API requires X-Amagi-Oidc-Source or default_oidc_source".to_owned(),
        })
}

#[derive(Debug)]
enum SyncApiError {
    Sync(SyncError),
    Auth(AuthError),
    Unauthenticated { message: String },
    Forbidden { message: String },
    BadRequest { code: &'static str, message: String },
}

#[derive(Debug, Serialize)]
struct SyncErrorResponse {
    code: &'static str,
    message: String,
    source: Option<String>,
}

impl From<SyncError> for SyncApiError {
    fn from(value: SyncError) -> Self {
        Self::Sync(value)
    }
}

impl From<AuthError> for SyncApiError {
    fn from(value: AuthError) -> Self {
        Self::Auth(value)
    }
}

impl IntoResponse for SyncApiError {
    fn into_response(self) -> Response {
        let (status, code, message, source) = match self {
            Self::Sync(error) => (
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
            Json(SyncErrorResponse {
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
    use amagi_bookmarks::{CreateLibraryRequest, LibraryKind};
    use amagi_config::ApiServerConfig;
    use amagi_db::{CurrentUserId, entities::users, set_current_user_id};
    use amagi_sync::{
        BrowserClientRegistrationRequest, DeviceRegistrationRequest, RegisterClientRequest,
    };
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
    async fn sync_routes_require_bearer_token() {
        let app = build_app(build_state(sample_config(None)).await);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/clients/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"device":{"deviceId":null,"deviceName":"My Mac","deviceType":"desktop","platform":"macos"},"browserClient":{"browserFamily":"chrome","browserProfileName":"Default","extensionInstanceId":"ext-1","capabilities":{}}}"#,
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn sync_routes_return_unauthorized_when_bearer_has_no_binding() {
        let mut state = build_state(sample_config(None)).await;
        state.dashboard_principal_override = Some(DashboardPrincipalOverride::Unbound);
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/session/start")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"browserClientId":"00000000-0000-0000-0000-000000000000","preferredProfileId":null,"localCapabilitySummary":{}}"#,
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn sync_routes_hide_foreign_browser_client_ids() {
        let postgres = start_amagi_postgres().await;
        let user_a = Uuid::now_v7();
        let user_b = Uuid::now_v7();
        let mut state = build_state(sample_config(Some(postgres.database_url()))).await;
        insert_user(&state, user_a).await;
        insert_user(&state, user_b).await;

        let registered = state
            .sync
            .register_client(
                user_a,
                &RegisterClientRequest {
                    device: DeviceRegistrationRequest {
                        device_id: None,
                        device_name: "Mac".to_owned(),
                        device_type: "desktop".to_owned(),
                        platform: "macos".to_owned(),
                    },
                    browser_client: BrowserClientRegistrationRequest {
                        browser_family: "chrome".to_owned(),
                        browser_profile_name: Some("Default".to_owned()),
                        extension_instance_id: "ext-a".to_owned(),
                        capabilities: json!({ "can_read_bookmarks": true }),
                    },
                },
            )
            .await
            .expect("registration succeeds");

        state.dashboard_principal_override =
            Some(DashboardPrincipalOverride::Bound { user_id: user_b });
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/session/start")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "browserClientId": registered.browser_client.id,
                            "preferredProfileId": null,
                            "localCapabilitySummary": {},
                        }))
                        .expect("request serializes"),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], "browser_client_not_found");
    }

    #[tokio::test]
    async fn sync_routes_support_register_session_preview_and_apply() {
        let postgres = start_amagi_postgres().await;
        let user_id = Uuid::now_v7();
        let mut state = build_state(sample_config(Some(postgres.database_url()))).await;
        insert_user(&state, user_id).await;
        state.dashboard_principal_override = Some(DashboardPrincipalOverride::Bound { user_id });

        let library = state
            .bookmarks
            .create_library(
                user_id,
                &CreateLibraryRequest {
                    name: "Default".to_owned(),
                    kind: LibraryKind::Normal,
                },
            )
            .await
            .expect("library creates");
        let library_id = library.library.id.clone();
        let root_id = library.nodes[0].id.clone();

        let app = build_app(state);
        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/clients/register")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "device": {
                                "deviceId": null,
                                "deviceName": "My Mac",
                                "deviceType": "desktop",
                                "platform": "macos"
                            },
                            "browserClient": {
                                "browserFamily": "chrome",
                                "browserProfileName": "Default",
                                "extensionInstanceId": "ext-123",
                                "capabilities": {
                                    "canReadBookmarks": true,
                                    "canWriteBookmarks": true
                                }
                            }
                        }))
                        .expect("request serializes"),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(register_response.status(), StatusCode::CREATED);
        let register_payload = response_json(register_response).await;

        let session_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/session/start")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "browserClientId": register_payload["browserClient"]["id"],
                            "preferredProfileId": null,
                            "localCapabilitySummary": {}
                        }))
                        .expect("request serializes"),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(session_response.status(), StatusCode::OK);
        let session_payload = response_json(session_response).await;
        assert_eq!(session_payload["libraries"][0]["id"], library_id);

        let preview_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/preview")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "browserClientId": register_payload["browserClient"]["id"],
                            "profileId": register_payload["defaultProfile"]["id"],
                            "libraryId": library_id,
                            "baseClock": 1,
                            "localSnapshotSummary": { "rootHash": "current" },
                            "localMutations": [
                                {
                                    "clientMutationId": "mutation-create",
                                    "op": "create",
                                    "serverNodeId": null,
                                    "clientExternalId": "local-1",
                                    "parentServerNodeId": root_id,
                                    "parentClientExternalId": null,
                                    "nodeType": "bookmark",
                                    "title": "Example",
                                    "url": "https://example.com",
                                    "sortKey": null
                                }
                            ]
                        }))
                        .expect("request serializes"),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(preview_response.status(), StatusCode::OK);
        let preview_payload = response_json(preview_response).await;
        assert_eq!(preview_payload["summary"]["localToServerAccepted"], 1);

        let apply_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sync/apply")
                    .header("authorization", "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "previewId": preview_payload["previewId"],
                            "confirm": true
                        }))
                        .expect("request serializes"),
                    ))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(apply_response.status(), StatusCode::OK);
        let apply_payload = response_json(apply_response).await;
        assert_eq!(apply_payload["applied"], true);
        assert_eq!(apply_payload["newClock"], 2);
        assert_eq!(
            apply_payload["createdMappings"]
                .as_array()
                .expect("array")
                .len(),
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

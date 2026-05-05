use amagi_config::DatabaseConfig;
use amagi_db::{
    AuthLookupIdentity, CurrentUserId, DatabaseService, DatabaseState,
    current_auth_lookup_identity, current_user_id, set_auth_lookup_identity, set_current_user_id,
};
use amagi_test_utils::postgres::start_amagi_postgres;
use sea_orm::TransactionTrait;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn postgres_runtime_auto_migrate_and_rls_helper_work() {
    let postgres = start_amagi_postgres().await;

    let config: DatabaseConfig = serde_json::from_value(json!({
        "url": postgres.database_url(),
        "auto_migrate": true,
    }))
    .expect("test database config deserializes");
    let service = DatabaseService::initialize(&config).await;

    let readiness = service.readiness_report().await;
    assert_eq!(readiness.state, DatabaseState::Ready, "{readiness:?}");

    let runtime = service.runtime().expect("database runtime is available");
    let txn = runtime
        .connection()
        .begin()
        .await
        .expect("transaction starts");
    let user_id = CurrentUserId::new(Uuid::now_v7());

    set_current_user_id(&txn, user_id)
        .await
        .expect("RLS helper stores current user ID");
    let auth_lookup_identity =
        AuthLookupIdentity::for_oidc_identity_key("primary", "oidc-sub-123", "oidc-user-123");
    set_auth_lookup_identity(&txn, &auth_lookup_identity)
        .await
        .expect("auth lookup helper stores lookup identity");

    assert_eq!(
        current_user_id(&txn).await.expect("user id reads back"),
        Some(user_id.into_uuid())
    );
    assert_eq!(
        current_auth_lookup_identity(&txn)
            .await
            .expect("auth lookup identity reads back"),
        Some(auth_lookup_identity)
    );

    txn.rollback().await.expect("transaction rolls back");
}

use std::sync::Arc;

use amagi_auth::{
    AccountBindingResolution, AuthAuditEventType, AuthFacadeService, ExternalOidcIdentity,
    OidcIdentityClaim,
};
use amagi_config::ApiServerConfig;
use amagi_db::{CurrentUserId, DatabaseService, entities::audit_events, set_current_user_id};
use amagi_securitydept::{AuthRuntime, VerifiedBearerPrincipalFacts};
use amagi_test_utils::postgres::start_amagi_postgres;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

fn sample_config(database_url: String) -> ApiServerConfig {
    let mut config = ApiServerConfig {
        default_oidc_source: Some("primary".to_owned()),
        ..ApiServerConfig::default()
    };

    config.oidc_sources.insert(
        "primary".to_owned(),
        serde_json::from_value(json!({
            "oidc": {
                "issuer_url": "https://issuer.primary",
                "well_known_url": "https://issuer.primary/.well-known/openid-configuration",
                "client_id": "interactive-client",
                "client_secret": "backend-secret"
            },
            "access_token_substrate": {
                "audiences": ["api://amagi"]
            }
        }))
        .expect("oidc source config parses"),
    );

    config.database = serde_json::from_value(json!({
        "url": database_url,
        "auto_migrate": true,
    }))
    .expect("database config parses");

    config
}

#[tokio::test]
async fn account_binding_repository_lookup_insert_and_reuse_work_with_testcontainer() {
    let postgres = start_amagi_postgres().await;
    let config = sample_config(postgres.database_url().to_owned());
    let database = DatabaseService::initialize(&config.database).await;
    assert!(
        database.startup_issue().is_none(),
        "database startup should be clean"
    );
    let service = AuthFacadeService::new(
        Arc::new(config.clone()),
        AuthRuntime::from_api_config(&config),
        database.clone(),
    );
    let external_id = format!("subject-{}", Uuid::now_v7());
    let claims_snapshot = json!({
        "sub": external_id,
        "email": "user@example.com",
        "refresh_token": "must-not-appear",
        "client_secret": "must-not-appear"
    });

    let first = service
        .resolve_principal("primary", claims_snapshot.clone())
        .await
        .expect("principal resolves");
    assert!(matches!(first, AccountBindingResolution::Created(_)));

    let second = service
        .resolve_principal("primary", claims_snapshot.clone())
        .await
        .expect("principal reuses binding");
    assert!(matches!(second, AccountBindingResolution::Reused(_)));
    assert_eq!(first.principal().user_id(), second.principal().user_id());
    assert_eq!(
        first.principal().auth_user_id(),
        second.principal().auth_user_id()
    );

    let lookup_identity =
        ExternalOidcIdentity::new("primary", OidcIdentityClaim::Sub, claims_snapshot)
            .expect("lookup identity builds");
    let lookup = service
        .binding_repository()
        .lookup_by_external_identity(&lookup_identity)
        .await
        .expect("binding lookup succeeds")
        .expect("binding exists");
    assert_eq!(lookup.user_id(), first.principal().user_id());
    assert_eq!(lookup.auth_user_id(), first.principal().auth_user_id());
    assert_eq!(
        lookup.oidc_subject(),
        first.principal().external_identity().oidc_subject()
    );

    let subject_lookup = service
        .binding_repository()
        .lookup_by_oidc_subject(
            first.principal().external_identity().source_key(),
            first.principal().external_identity().oidc_subject(),
        )
        .await
        .expect("subject lookup succeeds")
        .expect("subject binding exists");
    assert_eq!(subject_lookup.user_id(), first.principal().user_id());

    let bearer_resolution = service
        .resolve_bearer_principal_from_facts(VerifiedBearerPrincipalFacts {
            source_key: "primary".to_owned(),
            subject: Some(external_id.clone()),
            issuer: Some("https://issuer.primary".to_owned()),
            audiences: vec!["api://amagi".to_owned()],
            scopes: vec!["bookmarks:read".to_owned()],
            authorized_party: Some("extension-web".to_owned()),
            claims: serde_json::Map::from_iter([(
                "email".to_owned(),
                json!("different@example.com"),
            )]),
        })
        .await
        .expect("bearer subject lookup succeeds");
    let bearer_principal = bearer_resolution
        .principal
        .expect("bearer subject should resolve existing binding");
    assert_eq!(
        bearer_principal.user_id,
        first.principal().user_id().to_string()
    );
    assert_eq!(
        bearer_principal.auth_user_id,
        first.principal().auth_user_id().to_string()
    );
    assert_eq!(bearer_principal.oidc_subject, external_id);

    let runtime = database.runtime().expect("database runtime is present");
    let txn = runtime
        .connection()
        .begin()
        .await
        .expect("audit query transaction starts");
    set_current_user_id(&txn, CurrentUserId::new(first.principal().user_id()))
        .await
        .expect("user context is set");

    let event_count = audit_events::Entity::find()
        .filter(audit_events::Column::UserId.eq(first.principal().user_id()))
        .count(&txn)
        .await
        .expect("audit count query succeeds");
    assert_eq!(event_count, 4);

    let payload = audit_events::Entity::find()
        .filter(audit_events::Column::UserId.eq(first.principal().user_id()))
        .filter(
            audit_events::Column::EventType
                .eq(AuthAuditEventType::OidcAccountBindingCreated.as_str()),
        )
        .one(&txn)
        .await
        .expect("audit payload query succeeds")
        .expect("audit payload row exists")
        .payload_json;
    let payload_text = payload.to_string();
    assert!(!payload_text.contains("refresh_token"));
    assert!(!payload_text.contains("client_secret"));

    let principal_resolved = audit_events::Entity::find()
        .filter(audit_events::Column::UserId.eq(first.principal().user_id()))
        .filter(audit_events::Column::EventType.eq(AuthAuditEventType::PrincipalResolved.as_str()))
        .one(&txn)
        .await
        .expect("principal resolved audit query succeeds");
    assert!(
        principal_resolved.is_some(),
        "principal resolved audit row should exist"
    );

    txn.rollback()
        .await
        .expect("audit query transaction rolls back");
}

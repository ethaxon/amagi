use amagi_bookmarks::{CreateLibraryRequest, CreateNodeRequest, LibraryKind, NodeType};
use amagi_config::DatabaseConfig;
use amagi_db::{
    CurrentUserId, DatabaseService,
    entities::{sync_previews, users},
    set_current_user_id,
};
use amagi_sync::{
    BrowserClientRegistrationRequest, CursorAckRequest, DeviceRegistrationRequest, FeedRequest,
    LocalMutationInput, RegisterClientRequest, SyncApplyRequest, SyncPreviewRequest, SyncService,
    SyncSessionStartRequest, UpdateSyncProfileRequest,
};
use amagi_test_utils::postgres::{StartedPostgres, start_amagi_postgres};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn sync_service_register_session_preview_apply_feed_and_ack_work() {
    let (_postgres, database, sync, bookmarks) = services().await;
    let user_a = Uuid::now_v7();
    let user_b = Uuid::now_v7();
    insert_user(&database, user_a).await;
    insert_user(&database, user_b).await;

    let register = sync
        .register_client(
            user_a,
            &RegisterClientRequest {
                device: DeviceRegistrationRequest {
                    device_id: None,
                    device_name: "My Mac".to_owned(),
                    device_type: "desktop".to_owned(),
                    platform: "macos".to_owned(),
                },
                browser_client: BrowserClientRegistrationRequest {
                    browser_family: "chrome".to_owned(),
                    browser_profile_name: Some("Default".to_owned()),
                    extension_instance_id: "ext-123".to_owned(),
                    capabilities: json!({
                        "can_read_bookmarks": true,
                        "can_write_bookmarks": true,
                    }),
                },
            },
        )
        .await
        .expect("client registration succeeds");
    assert_eq!(register.default_profile.mode, "manual");

    let register_again = sync
        .register_client(
            user_a,
            &RegisterClientRequest {
                device: DeviceRegistrationRequest {
                    device_id: Some(register.device.id.clone()),
                    device_name: "My Mac Updated".to_owned(),
                    device_type: "desktop".to_owned(),
                    platform: "macos".to_owned(),
                },
                browser_client: BrowserClientRegistrationRequest {
                    browser_family: "chrome".to_owned(),
                    browser_profile_name: Some("Work".to_owned()),
                    extension_instance_id: "ext-123".to_owned(),
                    capabilities: json!({
                        "can_read_bookmarks": true,
                        "can_write_bookmarks": true,
                        "supports_preview": true,
                    }),
                },
            },
        )
        .await
        .expect("repeat registration updates in place");
    assert_eq!(register.browser_client.id, register_again.browser_client.id);
    assert_eq!(
        register_again
            .browser_client
            .browser_profile_name
            .as_deref(),
        Some("Work")
    );

    let library = bookmarks
        .create_library(
            user_a,
            &CreateLibraryRequest {
                name: "Default".to_owned(),
                kind: LibraryKind::Normal,
            },
        )
        .await
        .expect("library creates");
    let library_id = library.library.id.clone();
    let root_id = library.nodes[0].id.clone();

    let session = sync
        .start_session(
            user_a,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect("session start succeeds");
    assert_eq!(session.selected_profile.mode, "manual");
    assert_eq!(session.available_profiles.len(), 1);
    assert_eq!(session.libraries.len(), 1);
    assert_eq!(session.libraries[0].projection, "include");

    let stale_preview = sync
        .preview(
            user_a,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 0,
                local_snapshot_summary: json!({ "rootHash": "old" }),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-stale".to_owned(),
                    op: "create".to_owned(),
                    server_node_id: None,
                    client_external_id: Some("local-stale".to_owned()),
                    parent_server_node_id: Some(root_id.clone()),
                    parent_client_external_id: None,
                    node_type: Some("bookmark".to_owned()),
                    title: Some("Stale".to_owned()),
                    url: Some("https://stale.example".to_owned()),
                    sort_key: None,
                }],
            },
        )
        .await
        .expect("stale preview still persists conflict response");
    assert_eq!(stale_preview.conflicts[0].conflict_type, "stale_base_clock");
    assert!(stale_preview.accepted_local_mutations.is_empty());

    let preview = sync
        .preview(
            user_a,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 1,
                local_snapshot_summary: json!({ "rootHash": "current" }),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-create".to_owned(),
                    op: "create".to_owned(),
                    server_node_id: None,
                    client_external_id: Some("local-1".to_owned()),
                    parent_server_node_id: Some(root_id.clone()),
                    parent_client_external_id: None,
                    node_type: Some("bookmark".to_owned()),
                    title: Some("Example".to_owned()),
                    url: Some("https://example.com".to_owned()),
                    sort_key: None,
                }],
            },
        )
        .await
        .expect("preview accepts create mutation");
    assert_eq!(preview.summary.local_to_server_accepted, 1);
    assert!(preview.conflicts.is_empty());

    let applied = sync
        .apply(
            user_a,
            &SyncApplyRequest {
                preview_id: preview.preview_id.clone(),
                confirm: true,
            },
        )
        .await
        .expect("apply succeeds");
    assert!(applied.applied);
    assert_eq!(applied.new_clock, 2);
    assert_eq!(applied.created_mappings.len(), 1);
    assert_eq!(applied.created_mappings[0].client_external_id, "local-1");

    let applied_again = sync
        .apply(
            user_a,
            &SyncApplyRequest {
                preview_id: preview.preview_id.clone(),
                confirm: true,
            },
        )
        .await
        .expect("re-applying same preview is idempotent");
    assert_eq!(applied_again.new_clock, 2);
    assert_eq!(applied_again.created_mappings.len(), 1);

    let feed = sync
        .feed(
            user_a,
            &FeedRequest {
                browser_client_id: register.browser_client.id.clone(),
                library_id: library_id.clone(),
                from_clock: 1,
                profile_id: Some(session.selected_profile.id.clone()),
                limit: Some(100),
            },
        )
        .await
        .expect("feed succeeds");
    assert_eq!(feed.to_clock, 2);
    assert_eq!(feed.current_clock, 2);
    assert_eq!(feed.server_ops.len(), 1);
    assert_eq!(feed.server_ops[0].op_type, "node.create");

    let ack = sync
        .ack_cursor(
            user_a,
            &CursorAckRequest {
                browser_client_id: register.browser_client.id.clone(),
                library_id: library_id.clone(),
                applied_clock: 2,
                last_ack_rev_id: Some(feed.server_ops[0].rev_id.clone()),
            },
        )
        .await
        .expect("cursor ack succeeds");
    assert_eq!(ack.cursor.last_applied_clock, 2);

    let ack_lower = sync
        .ack_cursor(
            user_a,
            &CursorAckRequest {
                browser_client_id: register.browser_client.id.clone(),
                library_id: library_id.clone(),
                applied_clock: 1,
                last_ack_rev_id: Some(feed.server_ops[0].rev_id.clone()),
            },
        )
        .await
        .expect("lower clock ack is idempotent and does not rewind");
    assert_eq!(ack_lower.cursor.last_applied_clock, 2);

    let other_register = sync
        .register_client(
            user_b,
            &RegisterClientRequest {
                device: DeviceRegistrationRequest {
                    device_id: None,
                    device_name: "Other".to_owned(),
                    device_type: "desktop".to_owned(),
                    platform: "linux".to_owned(),
                },
                browser_client: BrowserClientRegistrationRequest {
                    browser_family: "firefox".to_owned(),
                    browser_profile_name: None,
                    extension_instance_id: "ext-other".to_owned(),
                    capabilities: json!({ "can_read_bookmarks": true }),
                },
            },
        )
        .await
        .expect("other user registration succeeds");

    let other_user_attempt = sync
        .start_session(
            user_b,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await;
    assert!(matches!(
        other_user_attempt,
        Err(amagi_sync::SyncError::BrowserClientNotFound)
    ));

    let other_user_preview = sync
        .preview(
            user_b,
            &SyncPreviewRequest {
                browser_client_id: other_register.browser_client.id.clone(),
                profile_id: other_register.default_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 0,
                local_snapshot_summary: json!({}),
                local_mutations: vec![],
            },
        )
        .await;
    assert!(matches!(
        other_user_preview,
        Err(amagi_sync::SyncError::LibraryNotFound)
    ));
}

#[tokio::test]
async fn sync_service_rejects_cross_library_mutation_targets() {
    let (_postgres, database, sync, bookmarks) = services().await;
    let user_id = Uuid::now_v7();
    insert_user(&database, user_id).await;

    let register = register_client(&sync, user_id, "ext-cross").await;
    let library_a = create_normal_library(&bookmarks, user_id, "Library A").await;
    let library_b = create_normal_library(&bookmarks, user_id, "Library B").await;
    let library_a_id = library_a.library.id.clone();
    let library_b_id = library_b.library.id.clone();
    let library_b_root_id = library_b.nodes[0].id.clone();
    let b_node = bookmarks
        .create_node(
            user_id,
            Uuid::parse_str(&library_b_id).expect("library id is uuid"),
            &CreateNodeRequest {
                node_type: NodeType::Bookmark,
                parent_id: Some(Uuid::parse_str(&library_b_root_id).expect("root id is uuid")),
                title: "Only In B".to_owned(),
                url: Some("https://b.example".to_owned()),
                sort_key: None,
            },
        )
        .await
        .expect("library b node creates");

    let session = sync
        .start_session(
            user_id,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect("session start succeeds");

    let error = sync
        .preview(
            user_id,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_a_id.clone(),
                base_clock: 1,
                local_snapshot_summary: json!({}),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-cross-delete".to_owned(),
                    op: "delete".to_owned(),
                    server_node_id: Some(b_node.id.clone()),
                    client_external_id: None,
                    parent_server_node_id: None,
                    parent_client_external_id: None,
                    node_type: None,
                    title: None,
                    url: None,
                    sort_key: None,
                }],
            },
        )
        .await
        .expect_err("cross-library target is rejected");
    assert!(matches!(
        error,
        amagi_sync::SyncError::InvalidRequest {
            code: "node_not_in_preview_library",
            ..
        }
    ));

    let tree_a = bookmarks
        .tree(
            user_id,
            Uuid::parse_str(&library_a_id).expect("library a id is uuid"),
        )
        .await
        .expect("tree a loads");
    let tree_b = bookmarks
        .tree(
            user_id,
            Uuid::parse_str(&library_b_id).expect("library b id is uuid"),
        )
        .await
        .expect("tree b loads");
    assert_eq!(tree_a.library.current_revision_clock, 1);
    assert_eq!(tree_b.library.current_revision_clock, 2);
    assert!(tree_b.nodes.iter().any(|node| node.id == b_node.id));
}

#[tokio::test]
async fn sync_service_rejects_duplicate_client_external_id_create() {
    let (_postgres, _database, sync, bookmarks) = services().await;
    let user_id = Uuid::now_v7();
    let database = _database;
    insert_user(&database, user_id).await;

    let register = register_client(&sync, user_id, "ext-dup").await;
    let library = create_normal_library(&bookmarks, user_id, "Default").await;
    let library_id = library.library.id.clone();
    let root_id = library.nodes[0].id.clone();
    let session = sync
        .start_session(
            user_id,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect("session start succeeds");

    let preview = sync
        .preview(
            user_id,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 1,
                local_snapshot_summary: json!({}),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-create-1".to_owned(),
                    op: "create".to_owned(),
                    server_node_id: None,
                    client_external_id: Some("local-dup".to_owned()),
                    parent_server_node_id: Some(root_id.clone()),
                    parent_client_external_id: None,
                    node_type: Some("bookmark".to_owned()),
                    title: Some("First".to_owned()),
                    url: Some("https://first.example".to_owned()),
                    sort_key: None,
                }],
            },
        )
        .await
        .expect("first preview succeeds");
    sync.apply(
        user_id,
        &SyncApplyRequest {
            preview_id: preview.preview_id,
            confirm: true,
        },
    )
    .await
    .expect("first apply succeeds");

    let duplicate = sync
        .preview(
            user_id,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 2,
                local_snapshot_summary: json!({}),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-create-2".to_owned(),
                    op: "create".to_owned(),
                    server_node_id: None,
                    client_external_id: Some("local-dup".to_owned()),
                    parent_server_node_id: Some(root_id.clone()),
                    parent_client_external_id: None,
                    node_type: Some("bookmark".to_owned()),
                    title: Some("Second".to_owned()),
                    url: Some("https://second.example".to_owned()),
                    sort_key: None,
                }],
            },
        )
        .await
        .expect_err("duplicate clientExternalId create is rejected");
    assert!(matches!(
        duplicate,
        amagi_sync::SyncError::InvalidRequest {
            code: "mapping_already_exists",
            ..
        }
    ));

    let tree = bookmarks
        .tree(
            user_id,
            Uuid::parse_str(&library_id).expect("library id is uuid"),
        )
        .await
        .expect("tree loads");
    assert_eq!(tree.library.current_revision_clock, 2);
    assert_eq!(tree.nodes.len(), 2);
}

#[tokio::test]
async fn sync_service_replays_applied_preview_even_after_expiry() {
    let (_postgres, database, sync, bookmarks) = services().await;
    let user_id = Uuid::now_v7();
    insert_user(&database, user_id).await;

    let register = register_client(&sync, user_id, "ext-expiry").await;
    let library = create_normal_library(&bookmarks, user_id, "Default").await;
    let library_id = library.library.id.clone();
    let root_id = library.nodes[0].id.clone();
    let session = sync
        .start_session(
            user_id,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect("session start succeeds");

    let preview = sync
        .preview(
            user_id,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: session.selected_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 1,
                local_snapshot_summary: json!({}),
                local_mutations: vec![LocalMutationInput {
                    client_mutation_id: "mutation-create-expire".to_owned(),
                    op: "create".to_owned(),
                    server_node_id: None,
                    client_external_id: Some("local-expire".to_owned()),
                    parent_server_node_id: Some(root_id.clone()),
                    parent_client_external_id: None,
                    node_type: Some("bookmark".to_owned()),
                    title: Some("Expire Replay".to_owned()),
                    url: Some("https://expire.example".to_owned()),
                    sort_key: None,
                }],
            },
        )
        .await
        .expect("preview succeeds");
    let preview_id = Uuid::parse_str(&preview.preview_id).expect("preview id is uuid");

    let first_apply = sync
        .apply(
            user_id,
            &SyncApplyRequest {
                preview_id: preview.preview_id.clone(),
                confirm: true,
            },
        )
        .await
        .expect("first apply succeeds");

    expire_preview(&database, user_id, preview_id).await;

    let replay = sync
        .apply(
            user_id,
            &SyncApplyRequest {
                preview_id: preview.preview_id,
                confirm: true,
            },
        )
        .await
        .expect("applied preview remains idempotent after expiry");
    assert_eq!(replay, first_apply);

    let tree = bookmarks
        .tree(
            user_id,
            Uuid::parse_str(&library_id).expect("library id is uuid"),
        )
        .await
        .expect("tree loads");
    assert_eq!(tree.library.current_revision_clock, 2);

    let stored_preview = load_preview(&database, user_id, preview_id).await;
    assert_eq!(stored_preview.status, "applied");
    assert!(stored_preview.applied_at.is_some());
}

#[tokio::test]
async fn sync_service_rejects_disabled_profiles_for_runtime_selection() {
    let (_postgres, database, sync, bookmarks) = services().await;
    let user_id = Uuid::now_v7();
    insert_user(&database, user_id).await;

    let register = register_client(&sync, user_id, "ext-disabled-profile").await;
    let library = create_normal_library(&bookmarks, user_id, "Default").await;
    let library_id = library.library.id.clone();

    let disabled_profile = sync
        .create_profile(
            user_id,
            &amagi_sync::CreateSyncProfileRequest {
                name: "Disabled Manual".to_owned(),
                mode: "manual".to_owned(),
                default_direction: "pull".to_owned(),
                conflict_policy: "manual".to_owned(),
            },
        )
        .await
        .expect("second manual profile creates");

    let disabled_profile = sync
        .update_profile(
            user_id,
            Uuid::parse_str(&disabled_profile.id).expect("profile id is uuid"),
            &UpdateSyncProfileRequest {
                name: None,
                enabled: Some(false),
                default_direction: None,
                conflict_policy: None,
            },
        )
        .await
        .expect("second manual profile can be disabled");
    assert!(!disabled_profile.enabled);

    let session = sync
        .start_session(
            user_id,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: None,
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect("session start succeeds with enabled default profile");
    assert_eq!(session.available_profiles.len(), 1);
    assert_eq!(
        session.available_profiles[0].id,
        register.default_profile.id
    );
    assert_eq!(session.selected_profile.id, register.default_profile.id);

    let preferred_disabled = sync
        .start_session(
            user_id,
            &SyncSessionStartRequest {
                browser_client_id: register.browser_client.id.clone(),
                preferred_profile_id: Some(disabled_profile.id.clone()),
                local_capability_summary: json!({}),
            },
        )
        .await
        .expect_err("disabled preferred profile is rejected");
    assert!(matches!(
        preferred_disabled,
        amagi_sync::SyncError::ProfileDisabled
    ));

    let preview_disabled = sync
        .preview(
            user_id,
            &SyncPreviewRequest {
                browser_client_id: register.browser_client.id.clone(),
                profile_id: disabled_profile.id.clone(),
                library_id: library_id.clone(),
                base_clock: 1,
                local_snapshot_summary: json!({}),
                local_mutations: vec![],
            },
        )
        .await
        .expect_err("disabled profile cannot be used for preview");
    assert!(matches!(
        preview_disabled,
        amagi_sync::SyncError::ProfileDisabled
    ));

    let feed_disabled = sync
        .feed(
            user_id,
            &FeedRequest {
                browser_client_id: register.browser_client.id.clone(),
                library_id: library_id.clone(),
                from_clock: 0,
                profile_id: Some(disabled_profile.id),
                limit: Some(100),
            },
        )
        .await
        .expect_err("disabled profile cannot be used for feed");
    assert!(matches!(
        feed_disabled,
        amagi_sync::SyncError::ProfileDisabled
    ));
}

async fn services() -> (
    StartedPostgres,
    DatabaseService,
    SyncService,
    amagi_bookmarks::BookmarkService,
) {
    let postgres = start_amagi_postgres().await;
    let config: DatabaseConfig = serde_json::from_value(json!({
        "url": postgres.database_url(),
        "auto_migrate": true,
    }))
    .expect("database config parses");
    let database = DatabaseService::initialize(&config).await;
    let bookmarks = amagi_bookmarks::BookmarkService::new(database.clone());
    let sync = SyncService::new(database.clone(), bookmarks.clone());
    (postgres, database, sync, bookmarks)
}

async fn insert_user(database: &DatabaseService, user_id: Uuid) {
    let txn = database
        .runtime()
        .expect("database runtime is available")
        .connection()
        .begin()
        .await
        .expect("user transaction starts");
    set_current_user_id(&txn, CurrentUserId::new(user_id))
        .await
        .expect("current user id sets");
    users::ActiveModel {
        id: Set(user_id),
        status: Set("active".to_owned()),
        ..Default::default()
    }
    .insert(&txn)
    .await
    .expect("user inserts");
    txn.commit().await.expect("user transaction commits");
}

async fn register_client(
    sync: &SyncService,
    user_id: Uuid,
    extension_instance_id: &str,
) -> amagi_sync::RegisterClientResponse {
    sync.register_client(
        user_id,
        &RegisterClientRequest {
            device: DeviceRegistrationRequest {
                device_id: None,
                device_name: "My Mac".to_owned(),
                device_type: "desktop".to_owned(),
                platform: "macos".to_owned(),
            },
            browser_client: BrowserClientRegistrationRequest {
                browser_family: "chrome".to_owned(),
                browser_profile_name: Some("Default".to_owned()),
                extension_instance_id: extension_instance_id.to_owned(),
                capabilities: json!({
                    "can_read_bookmarks": true,
                    "can_write_bookmarks": true,
                }),
            },
        },
    )
    .await
    .expect("client registration succeeds")
}

async fn create_normal_library(
    bookmarks: &amagi_bookmarks::BookmarkService,
    user_id: Uuid,
    name: &str,
) -> amagi_bookmarks::LibraryTreeView {
    bookmarks
        .create_library(
            user_id,
            &CreateLibraryRequest {
                name: name.to_owned(),
                kind: LibraryKind::Normal,
            },
        )
        .await
        .expect("library creates")
}

async fn expire_preview(database: &DatabaseService, user_id: Uuid, preview_id: Uuid) {
    let txn = database
        .runtime()
        .expect("database runtime is available")
        .connection()
        .begin()
        .await
        .expect("preview update transaction starts");
    set_current_user_id(&txn, CurrentUserId::new(user_id))
        .await
        .expect("current user id sets");
    let preview = sync_previews::Entity::find_by_id(preview_id)
        .one(&txn)
        .await
        .expect("preview loads")
        .expect("preview exists");
    let mut active = preview.into_active_model();
    active.expires_at = Set((Utc::now() - Duration::minutes(30)).fixed_offset());
    active.update(&txn).await.expect("preview expiry updates");
    txn.commit()
        .await
        .expect("preview update transaction commits");
}

async fn load_preview(
    database: &DatabaseService,
    user_id: Uuid,
    preview_id: Uuid,
) -> sync_previews::Model {
    let txn = database
        .runtime()
        .expect("database runtime is available")
        .connection()
        .begin()
        .await
        .expect("preview load transaction starts");
    set_current_user_id(&txn, CurrentUserId::new(user_id))
        .await
        .expect("current user id sets");
    let preview = sync_previews::Entity::find_by_id(preview_id)
        .one(&txn)
        .await
        .expect("preview loads")
        .expect("preview exists");
    txn.rollback()
        .await
        .expect("preview load transaction rolls back");
    preview
}

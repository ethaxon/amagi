use std::time::Duration;

use amagi_bookmarks::{
    BookmarkError, BookmarkService, CreateLibraryRequest, CreateNodeRequest, LibraryKind,
    MoveNodeRequest, NodeType, RestoreNodeRequest, UpdateNodeRequest,
};
use amagi_config::DatabaseConfig;
use amagi_db::{CurrentUserId, DatabaseService, entities::users, set_current_user_id};
use amagi_test_utils::postgres::{StartedPostgres, start_amagi_postgres};
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn bookmark_service_vertical_slice_works_with_postgres_rls_and_revisions() {
    let (_postgres, database, service) = bookmark_service().await;
    let user_a = Uuid::now_v7();
    let user_b = Uuid::now_v7();
    insert_user(&database, user_a).await;
    insert_user(&database, user_b).await;

    let created = service
        .create_library(
            user_a,
            &CreateLibraryRequest {
                name: "Default".to_owned(),
                kind: LibraryKind::Normal,
            },
        )
        .await
        .expect("normal library creates");
    assert_eq!(created.library.kind, "normal");
    assert_eq!(created.library.current_revision_clock, 1);
    assert_eq!(created.nodes.len(), 1);
    let library_id = parse_uuid(&created.library.id);
    let root_id = parse_uuid(&created.nodes[0].id);
    assert_eq!(created.nodes[0].node_type, "folder");
    assert_eq!(created.nodes[0].parent_id, None);

    let first_feed = service
        .revisions(user_a, library_id, 0, 100)
        .await
        .expect("initial revisions load");
    assert_eq!(first_feed.revisions.len(), 1);
    assert_eq!(first_feed.revisions[0].logical_clock, 1);
    assert_eq!(first_feed.revisions[0].op_type, "library.create");

    let bookmark = service
        .create_node(
            user_a,
            library_id,
            &CreateNodeRequest {
                node_type: NodeType::Bookmark,
                parent_id: Some(root_id),
                title: "Example".to_owned(),
                url: Some("  https://example.com/path?q=1  ".to_owned()),
                sort_key: None,
            },
        )
        .await
        .expect("bookmark creates");
    assert_eq!(
        bookmark.url.as_deref(),
        Some("https://example.com/path?q=1")
    );
    let bookmark_id = parse_uuid(&bookmark.id);

    let folder = service
        .create_node(
            user_a,
            library_id,
            &CreateNodeRequest {
                node_type: NodeType::Folder,
                parent_id: Some(root_id),
                title: "Nested".to_owned(),
                url: None,
                sort_key: Some("m-folder".to_owned()),
            },
        )
        .await
        .expect("folder creates");
    let folder_id = parse_uuid(&folder.id);

    let separator = service
        .create_node(
            user_a,
            library_id,
            &CreateNodeRequest {
                node_type: NodeType::Separator,
                parent_id: Some(root_id),
                title: "---".to_owned(),
                url: None,
                sort_key: None,
            },
        )
        .await
        .expect("separator creates");
    let separator_id = parse_uuid(&separator.id);

    tokio::time::sleep(Duration::from_millis(5)).await;
    let updated = service
        .update_node(
            user_a,
            bookmark_id,
            &UpdateNodeRequest {
                title: Some("Example Updated".to_owned()),
                url: Some("https://example.com/updated".to_owned()),
            },
        )
        .await
        .expect("bookmark updates");
    assert_eq!(updated.title, "Example Updated");
    assert!(updated.updated_at > bookmark.updated_at);

    tokio::time::sleep(Duration::from_millis(5)).await;
    let moved = service
        .move_node(
            user_a,
            bookmark_id,
            &MoveNodeRequest {
                parent_id: folder_id,
                sort_key: Some("moved".to_owned()),
            },
        )
        .await
        .expect("bookmark moves");
    assert_eq!(moved.parent_id.as_deref(), Some(folder.id.as_str()));
    assert!(moved.updated_at > updated.updated_at);

    tokio::time::sleep(Duration::from_millis(5)).await;
    let deleted = service
        .delete_node(user_a, bookmark_id)
        .await
        .expect("bookmark logical deletes");
    assert!(deleted.is_deleted);
    assert!(deleted.updated_at > moved.updated_at);

    tokio::time::sleep(Duration::from_millis(5)).await;
    let restored = service
        .restore_node(user_a, bookmark_id, &RestoreNodeRequest::default())
        .await
        .expect("bookmark restores");
    assert!(!restored.is_deleted);
    assert!(restored.updated_at > deleted.updated_at);

    let tree = service
        .tree(user_a, library_id)
        .await
        .expect("tree loads for owner");
    assert_eq!(tree.library.current_revision_clock, 8);
    assert_eq!(tree.nodes.len(), 4);
    assert!(tree.nodes.iter().any(|node| node.node_type == "bookmark"));
    assert!(tree.nodes.iter().any(|node| node.node_type == "separator"));

    let delta = service
        .revisions(user_a, library_id, 4, 100)
        .await
        .expect("delta revisions load");
    assert_eq!(delta.from_clock, 4);
    assert_eq!(delta.to_clock, 8);
    assert_eq!(
        delta
            .revisions
            .iter()
            .map(|revision| revision.logical_clock)
            .collect::<Vec<_>>(),
        vec![5, 6, 7, 8]
    );

    assert!(
        service
            .list_libraries(user_b)
            .await
            .expect("other user can list their libraries")
            .is_empty()
    );
    assert!(matches!(
        service.tree(user_b, library_id).await,
        Err(BookmarkError::LibraryNotFound)
    ));

    assert!(matches!(
        service
            .create_node(
                user_a,
                library_id,
                &CreateNodeRequest {
                    node_type: NodeType::Bookmark,
                    parent_id: Some(separator_id),
                    title: "Invalid".to_owned(),
                    url: Some("https://invalid.example".to_owned()),
                    sort_key: None,
                },
            )
            .await,
        Err(BookmarkError::InvalidParent)
    ));

    let second_library = service
        .create_library(
            user_a,
            &CreateLibraryRequest {
                name: "Second".to_owned(),
                kind: LibraryKind::Normal,
            },
        )
        .await
        .expect("second library creates");
    let second_root_id = parse_uuid(&second_library.nodes[0].id);
    assert!(matches!(
        service
            .create_node(
                user_a,
                library_id,
                &CreateNodeRequest {
                    node_type: NodeType::Bookmark,
                    parent_id: Some(second_root_id),
                    title: "Cross library".to_owned(),
                    url: Some("https://invalid.example".to_owned()),
                    sort_key: None,
                },
            )
            .await,
        Err(BookmarkError::InvalidParent)
    ));

    assert!(matches!(
        service.delete_node(user_a, root_id).await,
        Err(BookmarkError::RootNodeImmutable)
    ));
    assert!(matches!(
        service
            .move_node(
                user_a,
                root_id,
                &MoveNodeRequest {
                    parent_id: folder_id,
                    sort_key: None,
                },
            )
            .await,
        Err(BookmarkError::RootNodeImmutable)
    ));
    assert!(matches!(
        service
            .update_node(
                user_a,
                root_id,
                &UpdateNodeRequest {
                    title: Some("Root rename".to_owned()),
                    url: None,
                },
            )
            .await,
        Err(BookmarkError::RootNodeImmutable)
    ));

    assert!(matches!(
        service
            .create_library(
                user_a,
                &CreateLibraryRequest {
                    name: "Vault".to_owned(),
                    kind: LibraryKind::Vault,
                },
            )
            .await,
        Err(BookmarkError::VaultNotSupportedInIter6)
    ));
}

async fn bookmark_service() -> (StartedPostgres, DatabaseService, BookmarkService) {
    let postgres = start_amagi_postgres().await;
    let config: DatabaseConfig = serde_json::from_value(json!({
        "url": postgres.database_url(),
        "auto_migrate": true,
    }))
    .expect("database config parses");
    let database = DatabaseService::initialize(&config).await;
    let service = BookmarkService::new(database.clone());
    (postgres, database, service)
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

fn parse_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("view id is a uuid")
}

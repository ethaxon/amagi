use amagi_db::entities::{bookmark_meta, bookmark_nodes, libraries, library_heads, node_revisions};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    BookmarkError, BookmarkNodeView, BookmarkResult, CreateLibraryRequest, CreateNodeRequest,
    LibraryKind, LibraryTreeView, LibraryView, MoveNodeRequest, NodeType, RevisionFeedView,
    RevisionView, UpdateNodeRequest,
};

const ROOT_SORT_KEY: &str = "root";
const ACTOR_USER: &str = "user";

pub struct BookmarkRepository;

impl BookmarkRepository {
    pub async fn list_libraries(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
    ) -> BookmarkResult<Vec<LibraryView>> {
        let libraries = libraries::Entity::find()
            .filter(libraries::Column::OwnerUserId.eq(owner_user_id))
            .order_by_asc(libraries::Column::CreatedAt)
            .all(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "list libraries",
            })?;

        let mut views = Vec::with_capacity(libraries.len());
        for library in libraries {
            let head = Self::library_head(txn, library.id).await?;
            views.push(library_view(library, head.current_revision_clock));
        }

        Ok(views)
    }

    pub async fn create_library(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        request: &CreateLibraryRequest,
    ) -> BookmarkResult<LibraryTreeView> {
        if request.kind != LibraryKind::Normal {
            return Err(BookmarkError::VaultNotSupportedInIter6);
        }

        let library_id = Uuid::now_v7();
        let root_id = Uuid::now_v7();
        let library = libraries::ActiveModel {
            id: Set(library_id),
            owner_user_id: Set(owner_user_id),
            kind: Set(LibraryKind::Normal.as_str().to_owned()),
            name: Set(request.name.trim().to_owned()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert library",
        })?;

        library_heads::ActiveModel {
            library_id: Set(library_id),
            current_revision_clock: Set(0),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert library head",
        })?;

        let root = bookmark_nodes::ActiveModel {
            id: Set(root_id),
            library_id: Set(library_id),
            node_type: Set(NodeType::Folder.as_str().to_owned()),
            parent_id: Set(None),
            sort_key: Set(ROOT_SORT_KEY.to_owned()),
            title: Set(request.name.trim().to_owned()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert library root node",
        })?;

        Self::insert_empty_meta(txn, root_id).await?;
        let revision = Self::append_revision(
            txn,
            library_id,
            root_id,
            owner_user_id,
            "library.create",
            json!({
                "library": {
                    "id": library_id,
                    "kind": library.kind,
                    "name": library.name,
                },
                "rootNode": node_payload(&root),
            }),
        )
        .await?;

        Ok(LibraryTreeView {
            library: library_view(library, revision.logical_clock),
            nodes: vec![node_view(root)],
        })
    }

    pub async fn tree(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        library_id: Uuid,
    ) -> BookmarkResult<LibraryTreeView> {
        let library = Self::owned_library(txn, owner_user_id, library_id).await?;
        let head = Self::library_head(txn, library_id).await?;
        let nodes = bookmark_nodes::Entity::find()
            .filter(bookmark_nodes::Column::LibraryId.eq(library_id))
            .order_by_asc(bookmark_nodes::Column::ParentId)
            .order_by_asc(bookmark_nodes::Column::SortKey)
            .all(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load library tree",
            })?
            .into_iter()
            .map(node_view)
            .collect();

        Ok(LibraryTreeView {
            library: library_view(library, head.current_revision_clock),
            nodes,
        })
    }

    pub async fn create_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        library_id: Uuid,
        request: &CreateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        Self::owned_library(txn, owner_user_id, library_id).await?;
        let parent_id = request.parent_id.ok_or(BookmarkError::InvalidParent)?;
        Self::folder_parent(txn, library_id, parent_id).await?;
        let url_normalized = request.node_type.normalized_url(request.url.as_deref())?;
        let node_id = Uuid::now_v7();
        let sort_key = request
            .sort_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(CreateNodeRequest::fallback_sort_key);

        let node = bookmark_nodes::ActiveModel {
            id: Set(node_id),
            library_id: Set(library_id),
            node_type: Set(request.node_type.as_str().to_owned()),
            parent_id: Set(Some(parent_id)),
            sort_key: Set(sort_key),
            title: Set(request.title.trim().to_owned()),
            url: Set(url_normalized.clone()),
            url_normalized: Set(url_normalized),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert bookmark node",
        })?;

        Self::insert_empty_meta(txn, node_id).await?;
        Self::append_revision(
            txn,
            library_id,
            node_id,
            owner_user_id,
            "node.create",
            json!({ "node": node_payload(&node) }),
        )
        .await?;

        Ok(node_view(node))
    }

    pub async fn update_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
        request: &UpdateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let node = Self::owned_node(txn, owner_user_id, node_id).await?;
        Self::ensure_not_root(&node)?;
        let node_type = NodeType::from_db(&node.node_type)?;
        let next_title = request
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(node.title.as_str())
            .to_owned();
        let next_url = if request.url.is_some() {
            node_type.normalized_url(request.url.as_deref())?
        } else {
            node.url.clone()
        };
        if node_type != NodeType::Bookmark && request.url.is_some() {
            return Err(BookmarkError::InvalidNodeType);
        }

        let updated = bookmark_nodes::ActiveModel {
            id: Set(node_id),
            title: Set(next_title),
            url: Set(next_url.clone()),
            url_normalized: Set(next_url),
            ..Default::default()
        }
        .update(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "update bookmark node",
        })?;

        Self::append_revision(
            txn,
            updated.library_id,
            node_id,
            owner_user_id,
            "node.update",
            json!({ "node": node_payload(&updated) }),
        )
        .await?;

        Ok(node_view(updated))
    }

    pub async fn move_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
        request: &MoveNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let node = Self::owned_node(txn, owner_user_id, node_id).await?;
        Self::ensure_not_root(&node)?;
        let parent = Self::folder_parent(txn, node.library_id, request.parent_id).await?;
        if parent.id == node_id || Self::is_descendant(txn, node_id, parent.id).await? {
            return Err(BookmarkError::InvalidParent);
        }
        let sort_key = request
            .sort_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(CreateNodeRequest::fallback_sort_key);

        let updated = bookmark_nodes::ActiveModel {
            id: Set(node_id),
            parent_id: Set(Some(parent.id)),
            sort_key: Set(sort_key),
            ..Default::default()
        }
        .update(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "move bookmark node",
        })?;

        Self::append_revision(
            txn,
            updated.library_id,
            node_id,
            owner_user_id,
            "node.move",
            json!({ "node": node_payload(&updated), "parentId": parent.id }),
        )
        .await?;

        Ok(node_view(updated))
    }

    pub async fn delete_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
    ) -> BookmarkResult<BookmarkNodeView> {
        Self::set_deleted(txn, owner_user_id, node_id, true, "node.delete").await
    }

    pub async fn restore_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
    ) -> BookmarkResult<BookmarkNodeView> {
        Self::set_deleted(txn, owner_user_id, node_id, false, "node.restore").await
    }

    pub async fn revisions(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        library_id: Uuid,
        after_clock: i64,
        limit: u64,
    ) -> BookmarkResult<RevisionFeedView> {
        Self::owned_library(txn, owner_user_id, library_id).await?;
        let limit = limit.clamp(1, 500);
        let revisions = node_revisions::Entity::find()
            .filter(node_revisions::Column::LibraryId.eq(library_id))
            .filter(node_revisions::Column::LogicalClock.gt(after_clock))
            .order_by_asc(node_revisions::Column::LogicalClock)
            .limit(limit)
            .all(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load node revisions",
            })?;
        let to_clock = revisions
            .last()
            .map(|revision| revision.logical_clock)
            .unwrap_or(after_clock);
        let next_cursor = if revisions.len() as u64 == limit {
            Some(to_clock)
        } else {
            None
        };

        Ok(RevisionFeedView {
            library_id: library_id.to_string(),
            from_clock: after_clock,
            to_clock,
            revisions: revisions.into_iter().map(revision_view).collect(),
            next_cursor,
        })
    }

    async fn set_deleted(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
        is_deleted: bool,
        op_type: &'static str,
    ) -> BookmarkResult<BookmarkNodeView> {
        let node = Self::owned_node(txn, owner_user_id, node_id).await?;
        Self::ensure_not_root(&node)?;

        let updated = bookmark_nodes::ActiveModel {
            id: Set(node_id),
            is_deleted: Set(is_deleted),
            ..Default::default()
        }
        .update(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "update bookmark node deleted state",
        })?;

        Self::append_revision(
            txn,
            updated.library_id,
            node_id,
            owner_user_id,
            op_type,
            json!({ "node": node_payload(&updated) }),
        )
        .await?;

        Ok(node_view(updated))
    }

    async fn owned_library(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        library_id: Uuid,
    ) -> BookmarkResult<libraries::Model> {
        libraries::Entity::find_by_id(library_id)
            .filter(libraries::Column::OwnerUserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load owned library",
            })?
            .ok_or(BookmarkError::LibraryNotFound)
    }

    async fn owned_node(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        node_id: Uuid,
    ) -> BookmarkResult<bookmark_nodes::Model> {
        let node = bookmark_nodes::Entity::find_by_id(node_id)
            .one(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load bookmark node",
            })?
            .ok_or(BookmarkError::NodeNotFound)?;
        Self::owned_library(txn, owner_user_id, node.library_id).await?;
        Ok(node)
    }

    async fn folder_parent(
        txn: &DatabaseTransaction,
        library_id: Uuid,
        parent_id: Uuid,
    ) -> BookmarkResult<bookmark_nodes::Model> {
        let parent = bookmark_nodes::Entity::find_by_id(parent_id)
            .filter(bookmark_nodes::Column::LibraryId.eq(library_id))
            .one(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load parent bookmark node",
            })?
            .ok_or(BookmarkError::InvalidParent)?;

        if parent.is_deleted || parent.node_type != NodeType::Folder.as_str() {
            return Err(BookmarkError::InvalidParent);
        }

        Ok(parent)
    }

    fn ensure_not_root(node: &bookmark_nodes::Model) -> BookmarkResult<()> {
        if node.parent_id.is_none() {
            return Err(BookmarkError::RootNodeImmutable);
        }
        Ok(())
    }

    async fn is_descendant(
        txn: &DatabaseTransaction,
        ancestor_id: Uuid,
        mut candidate_id: Uuid,
    ) -> BookmarkResult<bool> {
        for _ in 0..128 {
            let Some(candidate) = bookmark_nodes::Entity::find_by_id(candidate_id)
                .one(txn)
                .await
                .map_err(|_| BookmarkError::DatabaseQuery {
                    action: "walk bookmark ancestor chain",
                })?
            else {
                return Ok(false);
            };

            let Some(parent_id) = candidate.parent_id else {
                return Ok(false);
            };
            if parent_id == ancestor_id {
                return Ok(true);
            }
            candidate_id = parent_id;
        }

        Err(BookmarkError::InvalidParent)
    }

    async fn insert_empty_meta(txn: &DatabaseTransaction, node_id: Uuid) -> BookmarkResult<()> {
        bookmark_meta::ActiveModel {
            node_id: Set(node_id),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert bookmark metadata",
        })?;
        Ok(())
    }

    async fn library_head(
        txn: &DatabaseTransaction,
        library_id: Uuid,
    ) -> BookmarkResult<library_heads::Model> {
        library_heads::Entity::find_by_id(library_id)
            .one(txn)
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "load library head",
            })?
            .ok_or(BookmarkError::LibraryNotFound)
    }

    async fn append_revision(
        txn: &DatabaseTransaction,
        library_id: Uuid,
        node_id: Uuid,
        actor_id: Uuid,
        op_type: &'static str,
        payload: serde_json::Value,
    ) -> BookmarkResult<RevisionView> {
        let logical_clock = next_library_clock(txn, library_id).await?;
        let revision = node_revisions::ActiveModel {
            rev_id: Set(Uuid::now_v7()),
            library_id: Set(library_id),
            node_id: Set(node_id),
            actor_type: Set(ACTOR_USER.to_owned()),
            actor_id: Set(Some(actor_id)),
            op_type: Set(op_type.to_owned()),
            payload_json: Set(payload),
            logical_clock: Set(logical_clock),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "insert node revision",
        })?;

        Ok(revision_view(revision))
    }
}

async fn next_library_clock(txn: &DatabaseTransaction, library_id: Uuid) -> BookmarkResult<i64> {
    let row = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE library_heads SET current_revision_clock = current_revision_clock + 1, \
             updated_at = CURRENT_TIMESTAMP WHERE library_id = $1 RETURNING current_revision_clock",
            [library_id.into()],
        ))
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "advance library revision clock",
        })?
        .ok_or(BookmarkError::LibraryNotFound)?;

    row.try_get::<i64>("", "current_revision_clock")
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "read advanced library revision clock",
        })
}

fn library_view(library: libraries::Model, current_revision_clock: i64) -> LibraryView {
    LibraryView {
        id: library.id.to_string(),
        kind: library.kind,
        name: library.name,
        current_revision_clock,
        created_at: library.created_at.to_rfc3339(),
        updated_at: library.updated_at.to_rfc3339(),
    }
}

fn node_view(node: bookmark_nodes::Model) -> BookmarkNodeView {
    BookmarkNodeView {
        id: node.id.to_string(),
        library_id: node.library_id.to_string(),
        parent_id: node.parent_id.map(|id| id.to_string()),
        node_type: node.node_type,
        title: node.title,
        sort_key: node.sort_key,
        url: node.url,
        url_normalized: node.url_normalized,
        is_deleted: node.is_deleted,
        created_at: node.created_at.to_rfc3339(),
        updated_at: node.updated_at.to_rfc3339(),
    }
}

fn revision_view(revision: node_revisions::Model) -> RevisionView {
    RevisionView {
        rev_id: revision.rev_id.to_string(),
        library_id: revision.library_id.to_string(),
        node_id: revision.node_id.to_string(),
        actor_type: revision.actor_type,
        actor_id: revision.actor_id.map(|id| id.to_string()),
        op_type: revision.op_type,
        payload: revision.payload_json,
        logical_clock: revision.logical_clock,
        created_at: revision.created_at.to_rfc3339(),
    }
}

fn node_payload(node: &bookmark_nodes::Model) -> serde_json::Value {
    json!({
        "id": node.id,
        "libraryId": node.library_id,
        "parentId": node.parent_id,
        "nodeType": node.node_type,
        "title": node.title,
        "sortKey": node.sort_key,
        "url": node.url,
        "urlNormalized": node.url_normalized,
        "isDeleted": node.is_deleted,
    })
}

use amagi_db::{CurrentUserId, DatabaseService, set_current_user_id};
use sea_orm::{DatabaseTransaction, TransactionTrait};
use uuid::Uuid;

use crate::{
    BookmarkError, BookmarkNodeView, BookmarkResult, CreateLibraryRequest, CreateNodeRequest,
    LibraryTreeView, LibraryView, MoveNodeRequest, RestoreNodeRequest, RevisionFeedView,
    UpdateNodeRequest, repository::BookmarkRepository,
};

#[derive(Debug, Clone)]
pub struct BookmarkService {
    database: DatabaseService,
}

#[derive(Debug, Clone, Copy)]
pub struct BookmarkTxn<'a> {
    txn: &'a DatabaseTransaction,
    owner_user_id: Uuid,
}

impl BookmarkService {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub fn bind_txn<'a>(
        &self,
        txn: &'a DatabaseTransaction,
        owner_user_id: Uuid,
    ) -> BookmarkTxn<'a> {
        BookmarkTxn { txn, owner_user_id }
    }

    pub async fn list_libraries(&self, user_id: Uuid) -> BookmarkResult<Vec<LibraryView>> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self.bind_txn(&txn, user_id).list_libraries().await;
        finish_read_txn(txn, result).await
    }

    pub async fn create_library(
        &self,
        user_id: Uuid,
        request: &CreateLibraryRequest,
    ) -> BookmarkResult<LibraryTreeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self.bind_txn(&txn, user_id).create_library(request).await;
        finish_write_txn(txn, result).await
    }

    pub async fn tree(&self, user_id: Uuid, library_id: Uuid) -> BookmarkResult<LibraryTreeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self.bind_txn(&txn, user_id).tree(library_id).await;
        finish_read_txn(txn, result).await
    }

    pub async fn create_node(
        &self,
        user_id: Uuid,
        library_id: Uuid,
        request: &CreateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self
            .bind_txn(&txn, user_id)
            .create_node(library_id, request)
            .await;
        finish_write_txn(txn, result).await
    }

    pub async fn update_node(
        &self,
        user_id: Uuid,
        node_id: Uuid,
        request: &UpdateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self
            .bind_txn(&txn, user_id)
            .update_node(node_id, request)
            .await;
        finish_write_txn(txn, result).await
    }

    pub async fn move_node(
        &self,
        user_id: Uuid,
        node_id: Uuid,
        request: &MoveNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self
            .bind_txn(&txn, user_id)
            .move_node(node_id, request)
            .await;
        finish_write_txn(txn, result).await
    }

    pub async fn delete_node(
        &self,
        user_id: Uuid,
        node_id: Uuid,
    ) -> BookmarkResult<BookmarkNodeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self.bind_txn(&txn, user_id).delete_node(node_id).await;
        finish_write_txn(txn, result).await
    }

    pub async fn restore_node(
        &self,
        user_id: Uuid,
        node_id: Uuid,
        _request: &RestoreNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self.bind_txn(&txn, user_id).restore_node(node_id).await;
        finish_write_txn(txn, result).await
    }

    pub async fn revisions(
        &self,
        user_id: Uuid,
        library_id: Uuid,
        after_clock: i64,
        limit: u64,
    ) -> BookmarkResult<RevisionFeedView> {
        let txn = self.begin_owner_txn(user_id).await?;
        let result = self
            .bind_txn(&txn, user_id)
            .revisions(library_id, after_clock, limit)
            .await;
        finish_read_txn(txn, result).await
    }

    pub async fn begin_owner_txn(&self, user_id: Uuid) -> BookmarkResult<DatabaseTransaction> {
        let runtime = self
            .database
            .runtime()
            .ok_or(BookmarkError::DatabaseUnavailable)?;
        let txn = runtime
            .connection()
            .begin()
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "start bookmark transaction",
            })?;
        set_current_user_id(&txn, CurrentUserId::new(user_id))
            .await
            .map_err(|_| BookmarkError::DatabaseQuery {
                action: "set bookmark current user",
            })?;
        Ok(txn)
    }
}

impl<'a> BookmarkTxn<'a> {
    pub fn txn(&self) -> &'a DatabaseTransaction {
        self.txn
    }

    pub fn owner_user_id(&self) -> Uuid {
        self.owner_user_id
    }

    pub async fn list_libraries(&self) -> BookmarkResult<Vec<LibraryView>> {
        BookmarkRepository::list_libraries(self.txn, self.owner_user_id).await
    }

    pub async fn create_library(
        &self,
        request: &CreateLibraryRequest,
    ) -> BookmarkResult<LibraryTreeView> {
        BookmarkRepository::create_library(self.txn, self.owner_user_id, request).await
    }

    pub async fn tree(&self, library_id: Uuid) -> BookmarkResult<LibraryTreeView> {
        BookmarkRepository::tree(self.txn, self.owner_user_id, library_id).await
    }

    pub async fn create_node(
        &self,
        library_id: Uuid,
        request: &CreateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        BookmarkRepository::create_node(self.txn, self.owner_user_id, library_id, request).await
    }

    pub async fn update_node(
        &self,
        node_id: Uuid,
        request: &UpdateNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        BookmarkRepository::update_node(self.txn, self.owner_user_id, node_id, request).await
    }

    pub async fn move_node(
        &self,
        node_id: Uuid,
        request: &MoveNodeRequest,
    ) -> BookmarkResult<BookmarkNodeView> {
        BookmarkRepository::move_node(self.txn, self.owner_user_id, node_id, request).await
    }

    pub async fn delete_node(&self, node_id: Uuid) -> BookmarkResult<BookmarkNodeView> {
        BookmarkRepository::delete_node(self.txn, self.owner_user_id, node_id).await
    }

    pub async fn restore_node(&self, node_id: Uuid) -> BookmarkResult<BookmarkNodeView> {
        BookmarkRepository::restore_node(self.txn, self.owner_user_id, node_id).await
    }

    pub async fn revisions(
        &self,
        library_id: Uuid,
        after_clock: i64,
        limit: u64,
    ) -> BookmarkResult<RevisionFeedView> {
        BookmarkRepository::revisions(self.txn, self.owner_user_id, library_id, after_clock, limit)
            .await
    }
}

async fn finish_read_txn<T>(
    txn: DatabaseTransaction,
    result: BookmarkResult<T>,
) -> BookmarkResult<T> {
    txn.rollback()
        .await
        .map_err(|_| BookmarkError::DatabaseQuery {
            action: "rollback bookmark read transaction",
        })?;
    result
}

async fn finish_write_txn<T>(
    txn: DatabaseTransaction,
    result: BookmarkResult<T>,
) -> BookmarkResult<T> {
    match result {
        Ok(value) => {
            txn.commit()
                .await
                .map_err(|_| BookmarkError::DatabaseQuery {
                    action: "commit bookmark transaction",
                })?;
            Ok(value)
        }
        Err(error) => {
            txn.rollback()
                .await
                .map_err(|_| BookmarkError::DatabaseQuery {
                    action: "rollback bookmark transaction",
                })?;
            Err(error)
        }
    }
}

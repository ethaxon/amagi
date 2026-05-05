mod error;
mod model;
mod repository;
mod service;

pub use error::{BookmarkError, BookmarkResult};
pub use model::{
    BookmarkNodeView, CreateLibraryRequest, CreateNodeRequest, LibraryKind, LibraryTreeView,
    LibraryView, MoveNodeRequest, NodeType, RestoreNodeRequest, RevisionFeedView, RevisionView,
    UpdateNodeRequest,
};
pub use service::{BookmarkService, BookmarkTxn};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_type_validation_requires_urls_only_for_bookmarks() {
        assert!(
            NodeType::Bookmark
                .validate_url(Some("https://example.com"))
                .is_ok()
        );
        assert!(matches!(
            NodeType::Bookmark.validate_url(Some("  ")),
            Err(BookmarkError::InvalidUrl)
        ));
        assert!(matches!(
            NodeType::Folder.validate_url(Some("https://example.com")),
            Err(BookmarkError::InvalidNodeType)
        ));
        assert!(matches!(
            NodeType::Separator.validate_url(Some("https://example.com")),
            Err(BookmarkError::InvalidNodeType)
        ));
    }

    #[test]
    fn url_normalization_trims_without_accepting_empty_values() {
        assert_eq!(
            NodeType::Bookmark
                .normalized_url(Some("  HTTPS://Example.COM/path?q=1  "))
                .expect("url normalizes"),
            Some("HTTPS://Example.COM/path?q=1".to_owned())
        );
        assert!(matches!(
            NodeType::Bookmark.normalized_url(Some("   ")),
            Err(BookmarkError::InvalidUrl)
        ));
    }

    #[test]
    fn sort_key_fallback_is_non_empty_and_time_ordered() {
        let first = CreateNodeRequest::fallback_sort_key();
        let second = CreateNodeRequest::fallback_sort_key();

        assert!(first.starts_with('m'));
        assert!(second.starts_with('m'));
        assert!(!first.is_empty());
        assert!(first <= second);
    }

    #[test]
    fn error_codes_match_dashboard_contract() {
        assert_eq!(
            BookmarkError::RootNodeImmutable.code(),
            "root_node_immutable"
        );
        assert_eq!(BookmarkError::InvalidParent.code(), "invalid_parent");
        assert_eq!(
            BookmarkError::VaultNotSupportedInIter6.code(),
            "vault_not_supported_in_iter6"
        );
    }
}

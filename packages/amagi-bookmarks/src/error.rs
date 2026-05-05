use snafu::Snafu;

pub type BookmarkResult<T> = Result<T, BookmarkError>;

#[derive(Debug, Snafu)]
pub enum BookmarkError {
    #[snafu(display("bookmark database runtime is not available"))]
    DatabaseUnavailable,

    #[snafu(display("bookmark database operation failed during {action}"))]
    DatabaseQuery { action: &'static str },

    #[snafu(display("library was not found or is not visible to the current principal"))]
    LibraryNotFound,

    #[snafu(display("bookmark node was not found or is not visible to the current principal"))]
    NodeNotFound,

    #[snafu(display("node type is invalid for this operation"))]
    InvalidNodeType,

    #[snafu(display("parent node is invalid for this operation"))]
    InvalidParent,

    #[snafu(display("bookmark URL is invalid"))]
    InvalidUrl,

    #[snafu(display("root bookmark node is immutable"))]
    RootNodeImmutable,

    #[snafu(display("vault libraries require a security policy and are not supported in Iter6"))]
    VaultNotSupportedInIter6,

    #[snafu(display("dashboard principal is required for bookmark operations"))]
    Unauthenticated,

    #[snafu(display("current principal is not allowed to perform this bookmark operation"))]
    Forbidden,
}

impl BookmarkError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DatabaseUnavailable => "database_unavailable",
            Self::DatabaseQuery { .. } => "database_query_failed",
            Self::LibraryNotFound => "library_not_found",
            Self::NodeNotFound => "node_not_found",
            Self::InvalidNodeType => "invalid_node_type",
            Self::InvalidParent => "invalid_parent",
            Self::InvalidUrl => "invalid_url",
            Self::RootNodeImmutable => "root_node_immutable",
            Self::VaultNotSupportedInIter6 => "vault_not_supported_in_iter6",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
        }
    }

    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Unauthenticated => 401,
            Self::Forbidden => 403,
            Self::LibraryNotFound | Self::NodeNotFound => 404,
            Self::InvalidNodeType
            | Self::InvalidParent
            | Self::InvalidUrl
            | Self::RootNodeImmutable
            | Self::VaultNotSupportedInIter6 => 400,
            Self::DatabaseUnavailable => 503,
            Self::DatabaseQuery { .. } => 500,
        }
    }
}

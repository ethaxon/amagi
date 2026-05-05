use snafu::Snafu;

pub type SyncResult<T> = Result<T, SyncError>;

#[derive(Debug, Snafu)]
pub enum SyncError {
    #[snafu(display("sync database runtime is not available"))]
    DatabaseUnavailable,

    #[snafu(display("sync database operation failed during {action}"))]
    DatabaseQuery { action: &'static str },

    #[snafu(display("sync request is invalid: {message}"))]
    InvalidRequest { code: &'static str, message: String },

    #[snafu(display("sync principal is required"))]
    Unauthenticated,

    #[snafu(display("browser client was not found or is not visible to the current principal"))]
    BrowserClientNotFound,

    #[snafu(display("device was not found or is not visible to the current principal"))]
    DeviceNotFound,

    #[snafu(display("sync profile was not found or is not visible to the current principal"))]
    ProfileNotFound,

    #[snafu(display("library was not found or is not visible to the current principal"))]
    LibraryNotFound,

    #[snafu(display("sync preview was not found or is not visible to the current principal"))]
    PreviewNotFound,

    #[snafu(display("sync preview is expired and must be regenerated"))]
    PreviewExpired,

    #[snafu(display("sync preview is stale and must be regenerated"))]
    PreviewStale,

    #[snafu(display("sync apply requires explicit confirmation"))]
    ConfirmationRequired,

    #[snafu(display("vault sync is not supported in this iteration"))]
    VaultSyncNotSupported,
}

impl SyncError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DatabaseUnavailable => "database_unavailable",
            Self::DatabaseQuery { .. } => "database_query_failed",
            Self::InvalidRequest { code, .. } => code,
            Self::Unauthenticated => "unauthenticated",
            Self::BrowserClientNotFound => "browser_client_not_found",
            Self::DeviceNotFound => "device_not_found",
            Self::ProfileNotFound => "profile_not_found",
            Self::LibraryNotFound => "library_not_found",
            Self::PreviewNotFound => "preview_not_found",
            Self::PreviewExpired => "preview_expired",
            Self::PreviewStale => "preview_stale",
            Self::ConfirmationRequired => "confirmation_required",
            Self::VaultSyncNotSupported => "vault_sync_not_supported",
        }
    }

    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Unauthenticated => 401,
            Self::BrowserClientNotFound
            | Self::DeviceNotFound
            | Self::ProfileNotFound
            | Self::LibraryNotFound
            | Self::PreviewNotFound => 404,
            Self::DatabaseUnavailable => 503,
            Self::DatabaseQuery { .. } => 500,
            Self::PreviewExpired
            | Self::PreviewStale
            | Self::ConfirmationRequired
            | Self::VaultSyncNotSupported
            | Self::InvalidRequest { .. } => 400,
        }
    }
}

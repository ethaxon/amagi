use snafu::Snafu;

pub type DbResult<T, E = DbError> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupIssueKind {
    ConnectionFailed,
    MigrationFailed,
}

impl StartupIssueKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::ConnectionFailed => "connection_failed",
            Self::MigrationFailed => "migration_failed",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::ConnectionFailed => "failed to connect to configured database",
            Self::MigrationFailed => "automatic database migration failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Snafu)]
pub enum DbError {
    #[snafu(display("failed to execute database query"))]
    Query,

    #[snafu(display("failed to run automatic database migration"))]
    Migration,

    #[snafu(display("failed to start database transaction"))]
    Transaction,
}

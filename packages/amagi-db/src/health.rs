use serde::Serialize;

use crate::{error::StartupIssueKind, runtime::DatabaseService};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseState {
    NotConfigured,
    Connected,
    Ready,
    SchemaMissing,
    PingFailed,
    ConnectionFailed,
    MigrationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabaseReport {
    pub configured: bool,
    pub state: DatabaseState,
    pub auto_migrate: bool,
    pub detail: Option<&'static str>,
}

impl DatabaseService {
    pub fn health_report(&self) -> DatabaseReport {
        match (self.is_configured(), self.startup_issue()) {
            (false, _) => DatabaseReport {
                configured: false,
                state: DatabaseState::NotConfigured,
                auto_migrate: self.auto_migrate(),
                detail: Some("database is not configured"),
            },
            (true, Some(StartupIssueKind::ConnectionFailed)) => DatabaseReport {
                configured: true,
                state: DatabaseState::ConnectionFailed,
                auto_migrate: self.auto_migrate(),
                detail: Some(StartupIssueKind::ConnectionFailed.message()),
            },
            (true, Some(StartupIssueKind::MigrationFailed)) => DatabaseReport {
                configured: true,
                state: DatabaseState::MigrationFailed,
                auto_migrate: self.auto_migrate(),
                detail: Some(StartupIssueKind::MigrationFailed.message()),
            },
            (true, None) => DatabaseReport {
                configured: true,
                state: DatabaseState::Connected,
                auto_migrate: self.auto_migrate(),
                detail: None,
            },
        }
    }

    pub(crate) fn readiness_from_state(
        &self,
        state: DatabaseState,
        detail: Option<&'static str>,
    ) -> DatabaseReport {
        DatabaseReport {
            configured: self.is_configured(),
            state,
            auto_migrate: self.auto_migrate(),
            detail,
        }
    }
}

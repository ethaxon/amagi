use std::fmt;

use amagi_config::DatabaseConfig;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};

use crate::{
    error::{DbError, DbResult, StartupIssueKind},
    health::{DatabaseReport, DatabaseState},
    migrate,
};

#[derive(Clone)]
pub struct DbRuntime {
    connection: DatabaseConnection,
}

impl DbRuntime {
    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}

impl fmt::Debug for DbRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("DbRuntime").finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
pub struct DatabaseService {
    configured: bool,
    auto_migrate: bool,
    runtime: Option<DbRuntime>,
    startup_issue: Option<StartupIssueKind>,
}

impl fmt::Debug for DatabaseService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseService")
            .field("configured", &self.configured)
            .field("auto_migrate", &self.auto_migrate)
            .field("runtime_present", &self.runtime.is_some())
            .field(
                "startup_issue",
                &self.startup_issue.map(StartupIssueKind::code),
            )
            .finish()
    }
}

impl DatabaseService {
    pub async fn initialize(config: &DatabaseConfig) -> Self {
        let configured_url = config
            .url
            .as_ref()
            .map(|value| value.expose_secret().trim())
            .filter(|value| !value.is_empty());

        let Some(database_url) = configured_url else {
            return Self {
                configured: false,
                auto_migrate: bool::from(config.auto_migrate),
                runtime: None,
                startup_issue: None,
            };
        };

        let auto_migrate = bool::from(config.auto_migrate);
        let connection = match Database::connect(database_url).await {
            Ok(connection) => connection,
            Err(_) => {
                return Self {
                    configured: true,
                    auto_migrate,
                    runtime: None,
                    startup_issue: Some(StartupIssueKind::ConnectionFailed),
                };
            }
        };

        let runtime = DbRuntime { connection };
        let startup_issue = if auto_migrate {
            migrate::run_auto_migrate(&runtime)
                .await
                .err()
                .map(|_| StartupIssueKind::MigrationFailed)
        } else {
            None
        };

        Self {
            configured: true,
            auto_migrate,
            runtime: Some(runtime),
            startup_issue,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.configured
    }

    pub fn auto_migrate(&self) -> bool {
        self.auto_migrate
    }

    pub fn runtime(&self) -> Option<&DbRuntime> {
        self.runtime.as_ref()
    }

    pub fn startup_issue(&self) -> Option<StartupIssueKind> {
        self.startup_issue
    }

    pub async fn readiness_report(&self) -> DatabaseReport {
        if !self.configured {
            return self.readiness_from_state(
                DatabaseState::NotConfigured,
                Some("database is not configured"),
            );
        }

        if let Some(issue) = self.startup_issue {
            return self.readiness_from_state(
                match issue {
                    StartupIssueKind::ConnectionFailed => DatabaseState::ConnectionFailed,
                    StartupIssueKind::MigrationFailed => DatabaseState::MigrationFailed,
                },
                Some(issue.message()),
            );
        }

        let Some(runtime) = self.runtime() else {
            return self.readiness_from_state(
                DatabaseState::ConnectionFailed,
                Some(StartupIssueKind::ConnectionFailed.message()),
            );
        };

        if ping(runtime).await.is_err() {
            return self
                .readiness_from_state(DatabaseState::PingFailed, Some("database ping failed"));
        }

        match migrate::core_schema_ready(runtime.connection()).await {
            Ok(true) => self.readiness_from_state(DatabaseState::Ready, None),
            Ok(false) => self.readiness_from_state(
                DatabaseState::SchemaMissing,
                Some("core migration has not been applied"),
            ),
            Err(_) => self.readiness_from_state(
                DatabaseState::PingFailed,
                Some("database schema check failed"),
            ),
        }
    }
}

pub async fn ping(runtime: &DbRuntime) -> DbResult<()> {
    runtime
        .connection()
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT 1".to_owned(),
        ))
        .await
        .map(|_| ())
        .map_err(|_| DbError::Query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_service_reports_not_configured() {
        let service = DatabaseService::initialize(&DatabaseConfig::default()).await;
        let health = service.health_report();
        let readiness = service.readiness_report().await;

        assert_eq!(health.state, DatabaseState::NotConfigured);
        assert_eq!(readiness.state, DatabaseState::NotConfigured);
        assert!(!health.configured);
    }
}

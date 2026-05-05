use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;

use crate::{
    error::{DbError, DbResult},
    runtime::DbRuntime,
};

pub async fn run_auto_migrate(runtime: &DbRuntime) -> DbResult<()> {
    amagi_db_migration::Migrator::up(runtime.connection(), None)
        .await
        .map_err(|_| DbError::Migration)
}

pub async fn core_schema_ready(connection: &DatabaseConnection) -> DbResult<bool> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let row = connection
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('public.users') IS NOT NULL AS users_table_exists".to_owned(),
        ))
        .await
        .map_err(|_| DbError::Query)?;

    Ok(row
        .as_ref()
        .and_then(|value| value.try_get::<bool>("", "users_table_exists").ok())
        .unwrap_or(false))
}

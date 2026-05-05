pub mod entities;
mod error;
mod health;
mod migrate;
mod rls;
mod runtime;

pub use error::{DbError, DbResult, StartupIssueKind};
pub use health::{DatabaseReport, DatabaseState};
pub use migrate::{core_schema_ready, run_auto_migrate};
pub use rls::{
    AuthLookupIdentity, CurrentUserId, current_auth_lookup_identity, current_user_id,
    set_auth_lookup_identity, set_current_user_id,
};
pub use runtime::{DatabaseService, DbRuntime, ping};

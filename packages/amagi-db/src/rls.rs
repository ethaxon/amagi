use std::fmt;

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};
use uuid::Uuid;

use crate::error::{DbError, DbResult};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrentUserId(Uuid);

#[derive(Clone, PartialEq, Eq)]
pub struct AuthLookupIdentity {
    source_key: String,
    oidc_subject: String,
    oidc_identity_key: Option<String>,
}

impl CurrentUserId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl AuthLookupIdentity {
    pub fn for_oidc_identity_key(
        source_key: impl Into<String>,
        oidc_subject: impl Into<String>,
        oidc_identity_key: impl Into<String>,
    ) -> Self {
        Self {
            source_key: source_key.into(),
            oidc_subject: oidc_subject.into(),
            oidc_identity_key: Some(oidc_identity_key.into()),
        }
    }

    pub fn for_oidc_subject(
        source_key: impl Into<String>,
        oidc_subject: impl Into<String>,
    ) -> Self {
        Self {
            source_key: source_key.into(),
            oidc_subject: oidc_subject.into(),
            oidc_identity_key: None,
        }
    }

    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    pub fn oidc_subject(&self) -> &str {
        &self.oidc_subject
    }

    pub fn oidc_identity_key(&self) -> Option<&str> {
        self.oidc_identity_key.as_deref()
    }
}

impl fmt::Debug for CurrentUserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CurrentUserId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Debug for AuthLookupIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthLookupIdentity")
            .field("source_key", &self.source_key)
            .field("oidc_subject", &"<redacted>")
            .field("oidc_identity_key", &"<redacted>")
            .finish()
    }
}

pub async fn set_current_user_id(
    txn: &DatabaseTransaction,
    user_id: CurrentUserId,
) -> DbResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('amagi.current_user_id', $1, true)",
        [user_id.into_uuid().to_string().into()],
    ))
    .await
    .map(|_| ())
    .map_err(|_| DbError::Query)
}

pub async fn set_auth_lookup_identity(
    txn: &DatabaseTransaction,
    identity: &AuthLookupIdentity,
) -> DbResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('amagi.auth_oidc_source', $1, true), \
         set_config('amagi.auth_oidc_subject', $2, true), \
         set_config('amagi.auth_oidc_identity_key', $3, true)",
        [
            identity.source_key().to_owned().into(),
            identity.oidc_subject().to_owned().into(),
            identity
                .oidc_identity_key()
                .unwrap_or_default()
                .to_owned()
                .into(),
        ],
    ))
    .await
    .map(|_| ())
    .map_err(|_| DbError::Query)
}

pub async fn current_user_id(txn: &DatabaseTransaction) -> DbResult<Option<Uuid>> {
    let row = txn
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT NULLIF(current_setting('amagi.current_user_id', true), '')::uuid AS \
             current_user_id"
                .to_owned(),
        ))
        .await
        .map_err(|_| DbError::Query)?;

    Ok(row
        .as_ref()
        .and_then(|value| value.try_get::<Option<Uuid>>("", "current_user_id").ok())
        .flatten())
}

pub async fn current_auth_lookup_identity(
    txn: &DatabaseTransaction,
) -> DbResult<Option<AuthLookupIdentity>> {
    let row = txn
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT NULLIF(current_setting('amagi.auth_oidc_source', true), '') AS \
             auth_oidc_source, NULLIF(current_setting('amagi.auth_oidc_subject', true), '') AS \
             auth_oidc_subject, NULLIF(current_setting('amagi.auth_oidc_identity_key', true), '') \
             AS auth_oidc_identity_key"
                .to_owned(),
        ))
        .await
        .map_err(|_| DbError::Query)?;

    let Some(row) = row.as_ref() else {
        return Ok(None);
    };

    let source_key = row
        .try_get::<Option<String>>("", "auth_oidc_source")
        .ok()
        .flatten();
    let oidc_subject = row
        .try_get::<Option<String>>("", "auth_oidc_subject")
        .ok()
        .flatten();
    let oidc_identity_key = row
        .try_get::<Option<String>>("", "auth_oidc_identity_key")
        .ok()
        .flatten();

    Ok(match (source_key, oidc_subject) {
        (Some(source_key), Some(oidc_subject)) => {
            let mut identity = AuthLookupIdentity::for_oidc_subject(source_key, oidc_subject);
            identity.oidc_identity_key = oidc_identity_key;
            Some(identity)
        }
        _ => None,
    })
}

use amagi_db::{
    AuthLookupIdentity, CurrentUserId, DatabaseService,
    entities::{auth_users, oidc_account_bindings, users},
    set_auth_lookup_identity, set_current_user_id,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::{
    audit::AuthAuditWriter,
    error::{AuthError, AuthResult},
    principal::{AmagiPrincipal, ExternalOidcIdentity},
};

const USER_STATUS_ACTIVE: &str = "active";
const AUTH_USER_STATUS_ACTIVE: &str = "active";

#[derive(Debug, Clone)]
pub struct AccountBindingRepository {
    database: DatabaseService,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBindingRecord {
    auth_user_id: Uuid,
    user_id: Uuid,
    oidc_source: String,
    oidc_subject: String,
    oidc_identity_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountBindingResolution {
    Created(AmagiPrincipal),
    Reused(AmagiPrincipal),
}

impl AccountBindingRepository {
    pub fn new(database: DatabaseService) -> Self {
        Self { database }
    }

    pub async fn lookup_by_external_identity(
        &self,
        identity: &ExternalOidcIdentity,
    ) -> AuthResult<Option<AccountBindingRecord>> {
        let runtime = self
            .database
            .runtime()
            .ok_or(AuthError::DatabaseUnavailable)?;
        let txn = runtime
            .connection()
            .begin()
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "start account binding lookup transaction",
            })?;

        set_auth_lookup_identity(
            &txn,
            &AuthLookupIdentity::for_oidc_identity_key(
                identity.source_key(),
                identity.oidc_subject(),
                identity.oidc_identity_key(),
            ),
        )
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "set account binding lookup identity",
        })?;

        let record = self.lookup_in_txn(&txn, identity).await?;

        txn.rollback().await.map_err(|_| AuthError::DatabaseQuery {
            action: "rollback account binding lookup transaction",
        })?;

        Ok(record)
    }

    pub async fn resolve_or_create(
        &self,
        identity: &ExternalOidcIdentity,
        audit_writer: &AuthAuditWriter,
    ) -> AuthResult<AccountBindingResolution> {
        let runtime = self
            .database
            .runtime()
            .ok_or(AuthError::DatabaseUnavailable)?;
        let txn = runtime
            .connection()
            .begin()
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "start account binding transaction",
            })?;

        set_auth_lookup_identity(
            &txn,
            &AuthLookupIdentity::for_oidc_identity_key(
                identity.source_key(),
                identity.oidc_subject(),
                identity.oidc_identity_key(),
            ),
        )
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "set account binding lookup identity",
        })?;

        let resolution = if let Some(record) = self.lookup_in_txn(&txn, identity).await? {
            set_current_user_id(&txn, CurrentUserId::new(record.user_id))
                .await
                .map_err(|_| AuthError::DatabaseQuery {
                    action: "set current user for existing binding",
                })?;

            self.touch_binding(&txn, identity).await?;

            let principal = record.into_principal(identity.clone());
            let _ = audit_writer
                .record_account_binding_reused(&txn, &principal)
                .await?;
            let _ = audit_writer
                .record_principal_resolved(&txn, &principal)
                .await?;

            AccountBindingResolution::Reused(principal)
        } else {
            let principal = self.insert_new_binding(&txn, identity).await?;
            let _ = audit_writer
                .record_account_binding_created(&txn, &principal)
                .await?;
            let _ = audit_writer
                .record_principal_resolved(&txn, &principal)
                .await?;

            AccountBindingResolution::Created(principal)
        };

        txn.commit().await.map_err(|_| AuthError::DatabaseQuery {
            action: "commit account binding transaction",
        })?;

        Ok(resolution)
    }

    pub async fn lookup_by_oidc_subject(
        &self,
        source_key: &str,
        oidc_subject: &str,
    ) -> AuthResult<Option<AccountBindingRecord>> {
        let runtime = self
            .database
            .runtime()
            .ok_or(AuthError::DatabaseUnavailable)?;
        let txn = runtime
            .connection()
            .begin()
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "start oidc subject lookup transaction",
            })?;

        set_auth_lookup_identity(
            &txn,
            &AuthLookupIdentity::for_oidc_subject(source_key, oidc_subject),
        )
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "set oidc subject lookup identity",
        })?;

        let model = oidc_account_bindings::Entity::find()
            .filter(oidc_account_bindings::Column::OidcSource.eq(source_key))
            .filter(oidc_account_bindings::Column::OidcSubject.eq(oidc_subject))
            .one(&txn)
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "lookup oidc account binding by subject",
            })?;

        txn.rollback().await.map_err(|_| AuthError::DatabaseQuery {
            action: "rollback oidc subject lookup transaction",
        })?;

        Ok(model.map(AccountBindingRecord::from))
    }

    async fn lookup_in_txn(
        &self,
        txn: &DatabaseTransaction,
        identity: &ExternalOidcIdentity,
    ) -> AuthResult<Option<AccountBindingRecord>> {
        let model = oidc_account_bindings::Entity::find()
            .filter(oidc_account_bindings::Column::OidcSource.eq(identity.source_key()))
            .filter(oidc_account_bindings::Column::OidcIdentityKey.eq(identity.oidc_identity_key()))
            .one(txn)
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "lookup oidc account binding",
            })?;

        Ok(model.map(AccountBindingRecord::from))
    }

    async fn insert_new_binding(
        &self,
        txn: &DatabaseTransaction,
        identity: &ExternalOidcIdentity,
    ) -> AuthResult<AmagiPrincipal> {
        let user_id = Uuid::now_v7();
        let auth_user_id = Uuid::now_v7();
        let binding_id = Uuid::now_v7();

        set_current_user_id(txn, CurrentUserId::new(user_id))
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "set current user for new binding",
            })?;

        users::ActiveModel {
            id: Set(user_id),
            status: Set(USER_STATUS_ACTIVE.to_owned()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "insert user for new binding",
        })?;

        auth_users::ActiveModel {
            id: Set(auth_user_id),
            user_id: Set(user_id),
            status: Set(AUTH_USER_STATUS_ACTIVE.to_owned()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "insert auth_user for new binding",
        })?;

        oidc_account_bindings::ActiveModel {
            id: Set(binding_id),
            auth_user_id: Set(auth_user_id),
            user_id: Set(user_id),
            oidc_source: Set(identity.source_key().to_owned()),
            oidc_subject: Set(identity.oidc_subject().to_owned()),
            oidc_identity_key: Set(identity.oidc_identity_key().to_owned()),
            claims_snapshot_json: Set(identity.claims_snapshot().clone()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| AuthError::DatabaseQuery {
            action: "insert oidc account binding",
        })?;

        self.touch_binding(txn, identity).await?;

        Ok(AmagiPrincipal::new(auth_user_id, user_id, identity.clone()))
    }

    async fn touch_binding(
        &self,
        txn: &DatabaseTransaction,
        identity: &ExternalOidcIdentity,
    ) -> AuthResult<()> {
        oidc_account_bindings::Entity::update_many()
            .col_expr(
                oidc_account_bindings::Column::OidcSubject,
                Expr::value(identity.oidc_subject().to_owned()),
            )
            .col_expr(
                oidc_account_bindings::Column::ClaimsSnapshotJson,
                Expr::value(identity.claims_snapshot().clone()),
            )
            .col_expr(
                oidc_account_bindings::Column::LastSeenAt,
                Expr::cust("CURRENT_TIMESTAMP"),
            )
            .col_expr(
                oidc_account_bindings::Column::UpdatedAt,
                Expr::cust("CURRENT_TIMESTAMP"),
            )
            .filter(oidc_account_bindings::Column::OidcSource.eq(identity.source_key()))
            .filter(oidc_account_bindings::Column::OidcIdentityKey.eq(identity.oidc_identity_key()))
            .exec(txn)
            .await
            .map_err(|_| AuthError::DatabaseQuery {
                action: "touch oidc account binding",
            })?;

        Ok(())
    }
}

impl AccountBindingRecord {
    pub fn auth_user_id(&self) -> Uuid {
        self.auth_user_id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn oidc_source(&self) -> &str {
        &self.oidc_source
    }

    pub fn oidc_subject(&self) -> &str {
        &self.oidc_subject
    }

    fn into_principal(self, external_identity: ExternalOidcIdentity) -> AmagiPrincipal {
        AmagiPrincipal::new(self.auth_user_id, self.user_id, external_identity)
    }
}

impl From<oidc_account_bindings::Model> for AccountBindingRecord {
    fn from(value: oidc_account_bindings::Model) -> Self {
        Self {
            auth_user_id: value.auth_user_id,
            user_id: value.user_id,
            oidc_source: value.oidc_source,
            oidc_subject: value.oidc_subject,
            oidc_identity_key: value.oidc_identity_key,
        }
    }
}

impl AccountBindingResolution {
    pub fn principal(&self) -> &AmagiPrincipal {
        match self {
            Self::Created(principal) | Self::Reused(principal) => principal,
        }
    }
}

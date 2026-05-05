use sea_orm_migration::{
    prelude::*,
    sea_query::{Condition, Expr, IntoIden, PostgresQueryBuilder, Query},
};

use crate::defs::{
    AuditEvents, BookmarkMeta, BookmarkNodes, BrowserClients, Devices, Libraries,
    OidcAccountBindings, SyncProfiles,
};

pub async fn apply_rls_sql(
    manager: &SchemaManager<'_>,
    statements: Vec<String>,
) -> Result<(), DbErr> {
    for statement in statements {
        manager
            .get_connection()
            .execute_unprepared(&statement)
            .await?;
    }

    Ok(())
}

pub fn owner_scoped_policy_sql<T: IntoIden>(
    table: T,
    policy_name: &str,
    predicate: Condition,
) -> String {
    let table_name = quote_ident(table);
    let predicate_sql = render_condition(predicate);

    format!(
        "ALTER TABLE {table_name} ENABLE ROW LEVEL SECURITY; ALTER TABLE {table_name} FORCE ROW \
         LEVEL SECURITY; CREATE POLICY \"{policy_name}\" ON {table_name} USING ({predicate_sql}) \
         WITH CHECK ({predicate_sql});"
    )
}

pub fn select_policy_sql<T: IntoIden>(table: T, policy_name: &str, predicate: Condition) -> String {
    let table_name = quote_ident(table);
    let predicate_sql = render_condition(predicate);

    format!("CREATE POLICY \"{policy_name}\" ON {table_name} FOR SELECT USING ({predicate_sql});")
}

pub fn owner_match_condition<T: IntoIden + Copy, C: IntoIden + Copy>(
    table: T,
    owner_col: C,
) -> Condition {
    Condition::all().add(Expr::col((table, owner_col)).eq(current_user_id_expr()))
}

pub fn library_owner_exists_condition<T: IntoIden + Copy, C: IntoIden + Copy>(
    table: T,
    library_col: C,
) -> Condition {
    Condition::all().add(Expr::exists(
        Query::select()
            .expr(Expr::cust("1"))
            .from(Libraries::Table)
            .cond_where(
                Condition::all()
                    .add(
                        Expr::col((Libraries::Table, Libraries::Id))
                            .eq(Expr::col((table, library_col))),
                    )
                    .add(
                        Expr::col((Libraries::Table, Libraries::OwnerUserId))
                            .eq(current_user_id_expr()),
                    ),
            )
            .take(),
    ))
}

pub fn browser_client_owner_condition<T: IntoIden + Copy, C: IntoIden + Copy>(
    table: T,
    browser_client_col: C,
) -> Condition {
    Condition::all().add(Expr::exists(
        Query::select()
            .expr(Expr::cust("1"))
            .from(BrowserClients::Table)
            .cond_where(
                Condition::all()
                    .add(
                        Expr::col((BrowserClients::Table, BrowserClients::Id))
                            .eq(Expr::col((table, browser_client_col))),
                    )
                    .add(Expr::exists(
                        Query::select()
                            .expr(Expr::cust("1"))
                            .from(Devices::Table)
                            .cond_where(
                                Condition::all()
                                    .add(Expr::col((Devices::Table, Devices::Id)).eq(Expr::col((
                                        BrowserClients::Table,
                                        BrowserClients::DeviceId,
                                    ))))
                                    .add(
                                        Expr::col((Devices::Table, Devices::UserId))
                                            .eq(current_user_id_expr()),
                                    ),
                            )
                            .take(),
                    )),
            )
            .take(),
    ))
}

pub fn bookmark_node_owner_condition<T: IntoIden + Copy, C: IntoIden + Copy>(
    table: T,
    node_col: C,
) -> Condition {
    Condition::all().add(Expr::exists(
        Query::select()
            .expr(Expr::cust("1"))
            .from(BookmarkNodes::Table)
            .cond_where(
                Condition::all()
                    .add(
                        Expr::col((BookmarkNodes::Table, BookmarkNodes::Id))
                            .eq(Expr::col((table, node_col))),
                    )
                    .add(library_owner_exists_condition(
                        BookmarkNodes::Table,
                        BookmarkNodes::LibraryId,
                    )),
            )
            .take(),
    ))
}

pub fn bookmark_meta_owner_condition() -> Condition {
    bookmark_node_owner_condition(BookmarkMeta::Table, BookmarkMeta::NodeId)
}

pub fn sync_profile_owner_condition<T: IntoIden + Copy, C: IntoIden + Copy>(
    table: T,
    profile_col: C,
) -> Condition {
    Condition::all().add(Expr::exists(
        Query::select()
            .expr(Expr::cust("1"))
            .from(SyncProfiles::Table)
            .cond_where(
                Condition::all()
                    .add(
                        Expr::col((SyncProfiles::Table, SyncProfiles::Id))
                            .eq(Expr::col((table, profile_col))),
                    )
                    .add(
                        Expr::col((SyncProfiles::Table, SyncProfiles::UserId))
                            .eq(current_user_id_expr()),
                    ),
            )
            .take(),
    ))
}

pub fn audit_events_owner_condition() -> Condition {
    Condition::any()
        .add(owner_match_condition(
            AuditEvents::Table,
            AuditEvents::UserId,
        ))
        .add(
            Condition::all().add(Expr::exists(
                Query::select()
                    .expr(Expr::cust("1"))
                    .from(Devices::Table)
                    .cond_where(
                        Condition::all()
                            .add(
                                Expr::col((Devices::Table, Devices::Id))
                                    .eq(Expr::col((AuditEvents::Table, AuditEvents::DeviceId))),
                            )
                            .add(
                                Expr::col((Devices::Table, Devices::UserId))
                                    .eq(current_user_id_expr()),
                            ),
                    )
                    .take(),
            )),
        )
        .add(browser_client_owner_condition(
            AuditEvents::Table,
            AuditEvents::BrowserClientId,
        ))
        .add(library_owner_exists_condition(
            AuditEvents::Table,
            AuditEvents::LibraryId,
        ))
}

pub fn oidc_account_binding_lookup_condition() -> Condition {
    Condition::all()
        .add(
            Expr::col((OidcAccountBindings::Table, OidcAccountBindings::OidcSource))
                .eq(current_auth_oidc_source_expr()),
        )
        .add(
            Condition::any()
                .add(
                    Expr::col((
                        OidcAccountBindings::Table,
                        OidcAccountBindings::OidcIdentityKey,
                    ))
                    .eq(current_auth_oidc_identity_key_expr()),
                )
                .add(
                    Expr::col((OidcAccountBindings::Table, OidcAccountBindings::OidcSubject))
                        .eq(current_auth_oidc_subject_expr()),
                ),
        )
}

pub fn current_user_id_sql() -> &'static str {
    "NULLIF(current_setting('amagi.current_user_id', true), '')::uuid"
}

pub fn current_auth_oidc_source_sql() -> &'static str {
    "NULLIF(current_setting('amagi.auth_oidc_source', true), '')"
}

pub fn current_auth_oidc_subject_sql() -> &'static str {
    "NULLIF(current_setting('amagi.auth_oidc_subject', true), '')"
}

pub fn current_auth_oidc_identity_key_sql() -> &'static str {
    "NULLIF(current_setting('amagi.auth_oidc_identity_key', true), '')"
}

fn current_user_id_expr() -> Expr {
    Expr::cust(current_user_id_sql())
}

fn current_auth_oidc_source_expr() -> Expr {
    Expr::cust(current_auth_oidc_source_sql())
}

fn current_auth_oidc_subject_expr() -> Expr {
    Expr::cust(current_auth_oidc_subject_sql())
}

fn current_auth_oidc_identity_key_expr() -> Expr {
    Expr::cust(current_auth_oidc_identity_key_sql())
}

fn render_condition(condition: Condition) -> String {
    let sql = Query::select()
        .expr(Expr::cust("1"))
        .cond_where(condition)
        .to_string(PostgresQueryBuilder);

    sql.split_once(" WHERE ")
        .map(|(_, predicate)| predicate.to_owned())
        .expect("condition SQL contains WHERE")
}

fn quote_ident<T: IntoIden>(iden: T) -> String {
    format!("\"{}\"", iden.into_iden())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{Libraries, SyncCursors};

    #[test]
    fn policy_sql_wraps_seaquery_rendered_predicate() {
        let sql = owner_scoped_policy_sql(
            Libraries::Table,
            "libraries_owner_isolation",
            owner_match_condition(Libraries::Table, Libraries::OwnerUserId),
        );

        assert!(sql.contains("ENABLE ROW LEVEL SECURITY"));
        assert!(sql.contains("FORCE ROW LEVEL SECURITY"));
        assert!(sql.contains("current_setting('amagi.current_user_id', true)"));
        assert!(sql.contains("libraries_owner_isolation"));
    }

    #[test]
    fn browser_client_owner_predicate_renders_exists_subqueries() {
        let sql = owner_scoped_policy_sql(
            SyncCursors::Table,
            "sync_cursors_owner_isolation",
            browser_client_owner_condition(SyncCursors::Table, SyncCursors::BrowserClientId),
        );

        assert!(sql.contains("\"browser_clients\""));
        assert!(sql.contains("\"devices\""));
        assert!(sql.to_ascii_lowercase().contains("exists"));
        assert!(
            sql.contains("\"browser_clients\".\"id\" = \"sync_cursors\".\"browser_client_id\"")
        );
    }
}

#[test]
fn oidc_account_binding_lookup_policy_renders_session_contract() {
    let sql = select_policy_sql(
        OidcAccountBindings::Table,
        "oidc_account_bindings_auth_lookup",
        oidc_account_binding_lookup_condition(),
    );

    assert!(sql.contains("FOR SELECT"));
    assert!(sql.contains("amagi.auth_oidc_source"));
    assert!(sql.contains("amagi.auth_oidc_subject"));
    assert!(sql.contains("amagi.auth_oidc_identity_key"));
    assert!(sql.contains("oidc_account_bindings_auth_lookup"));
}

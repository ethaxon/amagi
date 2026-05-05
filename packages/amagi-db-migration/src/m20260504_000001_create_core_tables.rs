use sea_orm_migration::{prelude::*, schema::*, sea_query::Expr};

use crate::{
    defs::{
        AuditEvents, AuthUsers, BookmarkMeta, BookmarkNodes, BrowserClients, Devices, Libraries,
        LibraryHeads, NodeClientMappings, NodeRevisions, OidcAccountBindings, SyncConflicts,
        SyncCursors, SyncPreviews, SyncProfileRules, SyncProfileTargets, SyncProfiles, Users,
        VaultUnlockSessions,
    },
    rls::{
        apply_rls_sql, audit_events_owner_condition, bookmark_meta_owner_condition,
        bookmark_node_owner_condition, browser_client_owner_condition,
        library_owner_exists_condition, oidc_account_binding_lookup_condition,
        owner_match_condition, owner_scoped_policy_sql, select_policy_sql,
        sync_profile_owner_condition,
    },
    schema::{
        boolean_default_false, boolean_default_true, create_postgres_auto_update_ts_fn,
        create_postgres_auto_update_ts_trigger, desc_index, drop_postgres_auto_update_ts_fn,
        drop_postgres_auto_update_ts_trigger, gin_index, index, jsonb, jsonb_default_array,
        jsonb_default_object, pk_uuid_v7, shared_pk_uuid, text_array_default_empty, timestamptz,
        timestamptz_null, unique_index,
    },
};

macro_rules! fk {
    ($name:expr, $from_table:expr, $from_col:expr, $to_table:expr, $to_col:expr $(,)?) => {
        ForeignKey::create()
            .name($name)
            .from($from_table, $from_col)
            .to($to_table, $to_col)
    };
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in core_tables() {
            manager.create_table(table).await?;
        }

        for index in core_indexes() {
            manager.create_index(index).await?;
        }

        apply_auto_update_triggers(manager).await?;

        apply_rls(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_auto_update_triggers(manager).await?;

        for table in vec![
            AuditEvents::Table.into_iden(),
            VaultUnlockSessions::Table.into_iden(),
            SyncProfileRules::Table.into_iden(),
            SyncProfileTargets::Table.into_iden(),
            SyncProfiles::Table.into_iden(),
            SyncConflicts::Table.into_iden(),
            SyncPreviews::Table.into_iden(),
            NodeClientMappings::Table.into_iden(),
            SyncCursors::Table.into_iden(),
            NodeRevisions::Table.into_iden(),
            LibraryHeads::Table.into_iden(),
            BookmarkMeta::Table.into_iden(),
            BookmarkNodes::Table.into_iden(),
            Libraries::Table.into_iden(),
            BrowserClients::Table.into_iden(),
            Devices::Table.into_iden(),
            OidcAccountBindings::Table.into_iden(),
            AuthUsers::Table.into_iden(),
            Users::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(table).to_owned())
                .await?;
        }

        Ok(())
    }
}

async fn apply_auto_update_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    create_postgres_auto_update_ts_fn(manager, "updated_at").await?;

    for table_name in updated_at_tables() {
        create_postgres_auto_update_ts_trigger(manager, "updated_at", table_name).await?;
    }

    Ok(())
}

async fn drop_auto_update_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for table_name in updated_at_tables() {
        drop_postgres_auto_update_ts_trigger(manager, "updated_at", table_name).await?;
    }

    drop_postgres_auto_update_ts_fn(manager, "updated_at").await?;

    Ok(())
}

fn updated_at_tables() -> [&'static str; 14] {
    [
        "users",
        "auth_users",
        "oidc_account_bindings",
        "devices",
        "browser_clients",
        "libraries",
        "bookmark_nodes",
        "bookmark_meta",
        "library_heads",
        "sync_cursors",
        "node_client_mappings",
        "sync_profiles",
        "sync_profile_rules",
        "sync_previews",
    ]
}

fn core_tables() -> Vec<TableCreateStatement> {
    vec![
        users_table(),
        auth_users_table(),
        oidc_account_bindings_table(),
        devices_table(),
        browser_clients_table(),
        libraries_table(),
        bookmark_nodes_table(),
        bookmark_meta_table(),
        library_heads_table(),
        node_revisions_table(),
        sync_cursors_table(),
        node_client_mappings_table(),
        sync_conflicts_table(),
        sync_profiles_table(),
        sync_profile_targets_table(),
        sync_profile_rules_table(),
        sync_previews_table(),
        vault_unlock_sessions_table(),
        audit_events_table(),
    ]
}

fn core_indexes() -> Vec<IndexCreateStatement> {
    vec![
        index("idx_users_email", Users::Table, &[Users::Email]),
        index(
            "idx_oidc_account_bindings_auth_user",
            OidcAccountBindings::Table,
            &[OidcAccountBindings::AuthUserId],
        ),
        index(
            "idx_oidc_account_bindings_user",
            OidcAccountBindings::Table,
            &[OidcAccountBindings::UserId],
        ),
        index(
            "idx_oidc_account_bindings_source_subject",
            OidcAccountBindings::Table,
            &[
                OidcAccountBindings::OidcSource,
                OidcAccountBindings::OidcSubject,
            ],
        ),
        unique_index(
            "idx_oidc_account_bindings_source_identity_key_unique",
            OidcAccountBindings::Table,
            &[
                OidcAccountBindings::OidcSource,
                OidcAccountBindings::OidcIdentityKey,
            ],
        ),
        index(
            "idx_devices_user_platform",
            Devices::Table,
            &[Devices::UserId, Devices::Platform],
        ),
        index(
            "idx_devices_user_device_type",
            Devices::Table,
            &[Devices::UserId, Devices::DeviceType],
        ),
        index(
            "idx_browser_clients_device_family",
            BrowserClients::Table,
            &[BrowserClients::DeviceId, BrowserClients::BrowserFamily],
        ),
        unique_index(
            "idx_browser_clients_device_extension_unique",
            BrowserClients::Table,
            &[
                BrowserClients::DeviceId,
                BrowserClients::ExtensionInstanceId,
            ],
        ),
        index(
            "idx_libraries_owner_kind",
            Libraries::Table,
            &[Libraries::OwnerUserId, Libraries::Kind],
        ),
        index(
            "idx_bookmark_nodes_library_parent_deleted",
            BookmarkNodes::Table,
            &[
                BookmarkNodes::LibraryId,
                BookmarkNodes::ParentId,
                BookmarkNodes::IsDeleted,
            ],
        ),
        index(
            "idx_bookmark_nodes_library_url_normalized",
            BookmarkNodes::Table,
            &[BookmarkNodes::LibraryId, BookmarkNodes::UrlNormalized],
        ),
        index(
            "idx_bookmark_nodes_library_type_deleted",
            BookmarkNodes::Table,
            &[
                BookmarkNodes::LibraryId,
                BookmarkNodes::NodeType,
                BookmarkNodes::IsDeleted,
            ],
        ),
        gin_index(
            "idx_bookmark_meta_tags_gin",
            BookmarkMeta::Table,
            &[BookmarkMeta::Tags],
        ),
        index(
            "idx_bookmark_meta_starred",
            BookmarkMeta::Table,
            &[BookmarkMeta::Starred],
        ),
        index(
            "idx_node_revisions_library_clock",
            NodeRevisions::Table,
            &[NodeRevisions::LibraryId, NodeRevisions::LogicalClock],
        ),
        index(
            "idx_node_revisions_node_clock",
            NodeRevisions::Table,
            &[NodeRevisions::NodeId, NodeRevisions::LogicalClock],
        ),
        index(
            "idx_node_revisions_actor",
            NodeRevisions::Table,
            &[NodeRevisions::ActorType, NodeRevisions::ActorId],
        ),
        unique_index(
            "idx_node_revisions_library_clock_unique",
            NodeRevisions::Table,
            &[NodeRevisions::LibraryId, NodeRevisions::LogicalClock],
        ),
        unique_index(
            "idx_node_client_mappings_client_external_unique",
            NodeClientMappings::Table,
            &[
                NodeClientMappings::BrowserClientId,
                NodeClientMappings::ClientExternalId,
            ],
        ),
        index(
            "idx_sync_conflicts_browser_state",
            SyncConflicts::Table,
            &[SyncConflicts::BrowserClientId, SyncConflicts::State],
        ),
        index(
            "idx_sync_conflicts_library_state",
            SyncConflicts::Table,
            &[SyncConflicts::LibraryId, SyncConflicts::State],
        ),
        index(
            "idx_sync_profile_rules_profile_order",
            SyncProfileRules::Table,
            &[SyncProfileRules::ProfileId, SyncProfileRules::RuleOrder],
        ),
        desc_index(
            "idx_sync_previews_user_created_at",
            SyncPreviews::Table,
            SyncPreviews::UserId,
            SyncPreviews::CreatedAt,
        ),
        index(
            "idx_sync_previews_client_library_status",
            SyncPreviews::Table,
            &[
                SyncPreviews::BrowserClientId,
                SyncPreviews::LibraryId,
                SyncPreviews::Status,
            ],
        ),
        index(
            "idx_vault_unlock_sessions_user_library_expires",
            VaultUnlockSessions::Table,
            &[
                VaultUnlockSessions::UserId,
                VaultUnlockSessions::LibraryId,
                VaultUnlockSessions::ExpiresAt,
            ],
        ),
        desc_index(
            "idx_audit_events_user_created_at",
            AuditEvents::Table,
            AuditEvents::UserId,
            AuditEvents::CreatedAt,
        ),
        desc_index(
            "idx_audit_events_library_created_at",
            AuditEvents::Table,
            AuditEvents::LibraryId,
            AuditEvents::CreatedAt,
        ),
        desc_index(
            "idx_audit_events_type_created_at",
            AuditEvents::Table,
            AuditEvents::EventType,
            AuditEvents::CreatedAt,
        ),
    ]
}

fn users_table() -> TableCreateStatement {
    Table::create()
        .table(Users::Table)
        .if_not_exists()
        .col(pk_uuid_v7(Users::Id))
        .col(text_null(Users::Email))
        .col(text_null(Users::DisplayName))
        .col(text(Users::Status))
        .col(timestamptz(Users::CreatedAt))
        .col(timestamptz(Users::UpdatedAt))
        .to_owned()
}

fn auth_users_table() -> TableCreateStatement {
    Table::create()
        .table(AuthUsers::Table)
        .if_not_exists()
        .col(pk_uuid_v7(AuthUsers::Id))
        .col(uuid(AuthUsers::UserId).unique_key().take())
        .col(text(AuthUsers::Status))
        .col(timestamptz(AuthUsers::CreatedAt))
        .col(timestamptz(AuthUsers::UpdatedAt))
        .foreign_key(fk!(
            "fk_auth_users_user",
            AuthUsers::Table,
            AuthUsers::UserId,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn oidc_account_bindings_table() -> TableCreateStatement {
    Table::create()
        .table(OidcAccountBindings::Table)
        .if_not_exists()
        .col(pk_uuid_v7(OidcAccountBindings::Id))
        .col(uuid(OidcAccountBindings::AuthUserId))
        .col(uuid(OidcAccountBindings::UserId))
        .col(text(OidcAccountBindings::OidcSource))
        .col(text(OidcAccountBindings::OidcSubject))
        .col(text(OidcAccountBindings::OidcIdentityKey))
        .col(jsonb_default_object(
            OidcAccountBindings::ClaimsSnapshotJson,
        ))
        .col(timestamptz_null(OidcAccountBindings::LastSeenAt))
        .col(timestamptz(OidcAccountBindings::CreatedAt))
        .col(timestamptz(OidcAccountBindings::UpdatedAt))
        .foreign_key(fk!(
            "fk_oidc_account_bindings_auth_user",
            OidcAccountBindings::Table,
            OidcAccountBindings::AuthUserId,
            AuthUsers::Table,
            AuthUsers::Id,
        ))
        .foreign_key(fk!(
            "fk_oidc_account_bindings_user",
            OidcAccountBindings::Table,
            OidcAccountBindings::UserId,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn devices_table() -> TableCreateStatement {
    Table::create()
        .table(Devices::Table)
        .if_not_exists()
        .col(pk_uuid_v7(Devices::Id))
        .col(uuid(Devices::UserId))
        .col(text(Devices::DeviceName))
        .col(text(Devices::DeviceType))
        .col(text(Devices::Platform))
        .col(text(Devices::TrustLevel))
        .col(timestamptz_null(Devices::LastSeenAt))
        .col(timestamptz(Devices::CreatedAt))
        .col(timestamptz(Devices::UpdatedAt))
        .foreign_key(fk!(
            "fk_devices_user",
            Devices::Table,
            Devices::UserId,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn browser_clients_table() -> TableCreateStatement {
    Table::create()
        .table(BrowserClients::Table)
        .if_not_exists()
        .col(pk_uuid_v7(BrowserClients::Id))
        .col(uuid(BrowserClients::DeviceId))
        .col(text(BrowserClients::BrowserFamily))
        .col(text_null(BrowserClients::BrowserProfileName))
        .col(text(BrowserClients::ExtensionInstanceId))
        .col(jsonb_default_object(BrowserClients::CapabilitiesJson))
        .col(timestamptz_null(BrowserClients::LastSeenAt))
        .col(timestamptz(BrowserClients::CreatedAt))
        .col(timestamptz(BrowserClients::UpdatedAt))
        .foreign_key(fk!(
            "fk_browser_clients_device",
            BrowserClients::Table,
            BrowserClients::DeviceId,
            Devices::Table,
            Devices::Id,
        ))
        .to_owned()
}

fn libraries_table() -> TableCreateStatement {
    Table::create()
        .table(Libraries::Table)
        .if_not_exists()
        .col(pk_uuid_v7(Libraries::Id))
        .col(uuid(Libraries::OwnerUserId))
        .col(text(Libraries::Kind))
        .col(text(Libraries::Name))
        .col(uuid_null(Libraries::VisibilityPolicyId))
        .col(timestamptz(Libraries::CreatedAt))
        .col(timestamptz(Libraries::UpdatedAt))
        .check((
            "ck_libraries_kind",
            Expr::col(Libraries::Kind).is_in(["normal", "vault"]),
        ))
        .foreign_key(fk!(
            "fk_libraries_owner_user",
            Libraries::Table,
            Libraries::OwnerUserId,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn bookmark_nodes_table() -> TableCreateStatement {
    Table::create()
        .table(BookmarkNodes::Table)
        .if_not_exists()
        .col(pk_uuid_v7(BookmarkNodes::Id))
        .col(uuid(BookmarkNodes::LibraryId))
        .col(text(BookmarkNodes::NodeType))
        .col(uuid_null(BookmarkNodes::ParentId))
        .col(text(BookmarkNodes::SortKey))
        .col(text(BookmarkNodes::Title))
        .col(text_null(BookmarkNodes::Url))
        .col(text_null(BookmarkNodes::UrlNormalized))
        .col(text_null(BookmarkNodes::ContentHash))
        .col(boolean_default_false(BookmarkNodes::IsDeleted))
        .col(timestamptz(BookmarkNodes::CreatedAt))
        .col(timestamptz(BookmarkNodes::UpdatedAt))
        .check((
            "ck_bookmark_nodes_node_type",
            Expr::col(BookmarkNodes::NodeType).is_in(["folder", "bookmark", "separator"]),
        ))
        .check((
            "ck_bookmark_nodes_bookmark_url",
            Expr::col(BookmarkNodes::NodeType)
                .ne("bookmark")
                .or(Expr::col(BookmarkNodes::Url).is_not_null()),
        ))
        .foreign_key(fk!(
            "fk_bookmark_nodes_library",
            BookmarkNodes::Table,
            BookmarkNodes::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .foreign_key(fk!(
            "fk_bookmark_nodes_parent",
            BookmarkNodes::Table,
            BookmarkNodes::ParentId,
            BookmarkNodes::Table,
            BookmarkNodes::Id,
        ))
        .to_owned()
}

fn bookmark_meta_table() -> TableCreateStatement {
    Table::create()
        .table(BookmarkMeta::Table)
        .if_not_exists()
        .col(shared_pk_uuid(BookmarkMeta::NodeId))
        .col(text_null(BookmarkMeta::Description))
        .col(text_array_default_empty(BookmarkMeta::Tags))
        .col(text_null(BookmarkMeta::CanonicalUrl))
        .col(text_null(BookmarkMeta::PageTitle))
        .col(uuid_null(BookmarkMeta::FaviconAssetId))
        .col(text_null(BookmarkMeta::ReadingState))
        .col(boolean_default_false(BookmarkMeta::Starred))
        .col(jsonb_default_object(BookmarkMeta::ExtraJson))
        .col(timestamptz(BookmarkMeta::UpdatedAt))
        .foreign_key(fk!(
            "fk_bookmark_meta_node",
            BookmarkMeta::Table,
            BookmarkMeta::NodeId,
            BookmarkNodes::Table,
            BookmarkNodes::Id,
        ))
        .to_owned()
}

fn library_heads_table() -> TableCreateStatement {
    Table::create()
        .table(LibraryHeads::Table)
        .if_not_exists()
        .col(shared_pk_uuid(LibraryHeads::LibraryId))
        .col(big_integer(LibraryHeads::CurrentRevisionClock))
        .col(timestamptz(LibraryHeads::UpdatedAt))
        .foreign_key(fk!(
            "fk_library_heads_library",
            LibraryHeads::Table,
            LibraryHeads::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .to_owned()
}

fn node_revisions_table() -> TableCreateStatement {
    Table::create()
        .table(NodeRevisions::Table)
        .if_not_exists()
        .col(pk_uuid_v7(NodeRevisions::RevId))
        .col(uuid(NodeRevisions::LibraryId))
        .col(uuid(NodeRevisions::NodeId))
        .col(text(NodeRevisions::ActorType))
        .col(uuid_null(NodeRevisions::ActorId))
        .col(text(NodeRevisions::OpType))
        .col(jsonb(NodeRevisions::PayloadJson))
        .col(big_integer(NodeRevisions::LogicalClock))
        .col(timestamptz(NodeRevisions::CreatedAt))
        .foreign_key(fk!(
            "fk_node_revisions_library",
            NodeRevisions::Table,
            NodeRevisions::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .foreign_key(fk!(
            "fk_node_revisions_node",
            NodeRevisions::Table,
            NodeRevisions::NodeId,
            BookmarkNodes::Table,
            BookmarkNodes::Id,
        ))
        .to_owned()
}

fn sync_cursors_table() -> TableCreateStatement {
    Table::create()
        .table(SyncCursors::Table)
        .if_not_exists()
        .col(uuid(SyncCursors::BrowserClientId))
        .col(uuid(SyncCursors::LibraryId))
        .col(big_integer(SyncCursors::LastAppliedClock))
        .col(uuid_null(SyncCursors::LastAckRevId))
        .col(timestamptz_null(SyncCursors::LastSyncAt))
        .col(timestamptz(SyncCursors::UpdatedAt))
        .primary_key(
            Index::create()
                .name("pk_sync_cursors")
                .col(SyncCursors::BrowserClientId)
                .col(SyncCursors::LibraryId),
        )
        .foreign_key(fk!(
            "fk_sync_cursors_browser_client",
            SyncCursors::Table,
            SyncCursors::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_cursors_library",
            SyncCursors::Table,
            SyncCursors::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_cursors_last_ack_rev",
            SyncCursors::Table,
            SyncCursors::LastAckRevId,
            NodeRevisions::Table,
            NodeRevisions::RevId,
        ))
        .to_owned()
}

fn node_client_mappings_table() -> TableCreateStatement {
    Table::create()
        .table(NodeClientMappings::Table)
        .if_not_exists()
        .col(uuid(NodeClientMappings::BrowserClientId))
        .col(uuid(NodeClientMappings::ServerNodeId))
        .col(text(NodeClientMappings::ClientExternalId))
        .col(text_null(NodeClientMappings::LastSeenHash))
        .col(timestamptz(NodeClientMappings::UpdatedAt))
        .primary_key(
            Index::create()
                .name("pk_node_client_mappings")
                .col(NodeClientMappings::BrowserClientId)
                .col(NodeClientMappings::ServerNodeId),
        )
        .foreign_key(fk!(
            "fk_node_client_mappings_browser_client",
            NodeClientMappings::Table,
            NodeClientMappings::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .foreign_key(fk!(
            "fk_node_client_mappings_server_node",
            NodeClientMappings::Table,
            NodeClientMappings::ServerNodeId,
            BookmarkNodes::Table,
            BookmarkNodes::Id,
        ))
        .to_owned()
}

fn sync_conflicts_table() -> TableCreateStatement {
    Table::create()
        .table(SyncConflicts::Table)
        .if_not_exists()
        .col(pk_uuid_v7(SyncConflicts::Id))
        .col(uuid(SyncConflicts::BrowserClientId))
        .col(uuid(SyncConflicts::LibraryId))
        .col(text(SyncConflicts::ConflictType))
        .col(text(SyncConflicts::State))
        .col(text(SyncConflicts::Summary))
        .col(jsonb(SyncConflicts::DetailsJson))
        .col(timestamptz(SyncConflicts::CreatedAt))
        .col(timestamptz_null(SyncConflicts::ResolvedAt))
        .col(uuid_null(SyncConflicts::ResolvedBy))
        .foreign_key(fk!(
            "fk_sync_conflicts_browser_client",
            SyncConflicts::Table,
            SyncConflicts::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_conflicts_library",
            SyncConflicts::Table,
            SyncConflicts::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_conflicts_resolved_by",
            SyncConflicts::Table,
            SyncConflicts::ResolvedBy,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn sync_profiles_table() -> TableCreateStatement {
    Table::create()
        .table(SyncProfiles::Table)
        .if_not_exists()
        .col(pk_uuid_v7(SyncProfiles::Id))
        .col(uuid(SyncProfiles::UserId))
        .col(text(SyncProfiles::Name))
        .col(text(SyncProfiles::Mode))
        .col(text(SyncProfiles::DefaultDirection))
        .col(text(SyncProfiles::ConflictPolicy))
        .col(boolean_default_true(SyncProfiles::Enabled))
        .col(timestamptz(SyncProfiles::CreatedAt))
        .col(timestamptz(SyncProfiles::UpdatedAt))
        .check((
            "ck_sync_profiles_mode",
            Expr::col(SyncProfiles::Mode).is_in(["manual", "scheduled", "auto"]),
        ))
        .foreign_key(fk!(
            "fk_sync_profiles_user",
            SyncProfiles::Table,
            SyncProfiles::UserId,
            Users::Table,
            Users::Id,
        ))
        .to_owned()
}

fn sync_profile_targets_table() -> TableCreateStatement {
    Table::create()
        .table(SyncProfileTargets::Table)
        .if_not_exists()
        .col(pk_uuid_v7(SyncProfileTargets::Id))
        .col(uuid(SyncProfileTargets::ProfileId))
        .col(text_null(SyncProfileTargets::Platform))
        .col(text_null(SyncProfileTargets::DeviceType))
        .col(uuid_null(SyncProfileTargets::DeviceId))
        .col(text_null(SyncProfileTargets::BrowserFamily))
        .col(uuid_null(SyncProfileTargets::BrowserClientId))
        .col(timestamptz(SyncProfileTargets::CreatedAt))
        .foreign_key(fk!(
            "fk_sync_profile_targets_profile",
            SyncProfileTargets::Table,
            SyncProfileTargets::ProfileId,
            SyncProfiles::Table,
            SyncProfiles::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_profile_targets_device",
            SyncProfileTargets::Table,
            SyncProfileTargets::DeviceId,
            Devices::Table,
            Devices::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_profile_targets_browser_client",
            SyncProfileTargets::Table,
            SyncProfileTargets::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .to_owned()
}

fn sync_profile_rules_table() -> TableCreateStatement {
    Table::create()
        .table(SyncProfileRules::Table)
        .if_not_exists()
        .col(pk_uuid_v7(SyncProfileRules::Id))
        .col(uuid(SyncProfileRules::ProfileId))
        .col(integer(SyncProfileRules::RuleOrder))
        .col(text(SyncProfileRules::Action))
        .col(text(SyncProfileRules::MatcherType))
        .col(text(SyncProfileRules::MatcherValue))
        .col(jsonb_default_object(SyncProfileRules::OptionsJson))
        .col(timestamptz(SyncProfileRules::CreatedAt))
        .col(timestamptz(SyncProfileRules::UpdatedAt))
        .check((
            "ck_sync_profile_rules_action",
            Expr::col(SyncProfileRules::Action).is_in(["include", "exclude", "readonly"]),
        ))
        .foreign_key(fk!(
            "fk_sync_profile_rules_profile",
            SyncProfileRules::Table,
            SyncProfileRules::ProfileId,
            SyncProfiles::Table,
            SyncProfiles::Id,
        ))
        .to_owned()
}

fn sync_previews_table() -> TableCreateStatement {
    Table::create()
        .table(SyncPreviews::Table)
        .if_not_exists()
        .col(pk_uuid_v7(SyncPreviews::Id))
        .col(uuid(SyncPreviews::UserId))
        .col(uuid(SyncPreviews::BrowserClientId))
        .col(uuid(SyncPreviews::LibraryId))
        .col(big_integer(SyncPreviews::BaseClock))
        .col(big_integer(SyncPreviews::ToClock))
        .col(text(SyncPreviews::Status))
        .col(text(SyncPreviews::RequestHash))
        .col(jsonb_default_object(SyncPreviews::SummaryJson))
        .col(jsonb_default_array(SyncPreviews::ServerOpsJson))
        .col(jsonb_default_array(
            SyncPreviews::AcceptedLocalMutationsJson,
        ))
        .col(jsonb_default_array(SyncPreviews::ConflictsJson))
        .col(timestamptz(SyncPreviews::ExpiresAt))
        .col(timestamptz(SyncPreviews::CreatedAt))
        .col(timestamptz(SyncPreviews::UpdatedAt))
        .col(timestamptz_null(SyncPreviews::AppliedAt))
        .check((
            "ck_sync_previews_status",
            Expr::col(SyncPreviews::Status).is_in(["pending", "applied", "expired", "conflicted"]),
        ))
        .foreign_key(fk!(
            "fk_sync_previews_user",
            SyncPreviews::Table,
            SyncPreviews::UserId,
            Users::Table,
            Users::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_previews_browser_client",
            SyncPreviews::Table,
            SyncPreviews::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .foreign_key(fk!(
            "fk_sync_previews_library",
            SyncPreviews::Table,
            SyncPreviews::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .to_owned()
}

fn vault_unlock_sessions_table() -> TableCreateStatement {
    Table::create()
        .table(VaultUnlockSessions::Table)
        .if_not_exists()
        .col(pk_uuid_v7(VaultUnlockSessions::Id))
        .col(uuid(VaultUnlockSessions::UserId))
        .col(uuid(VaultUnlockSessions::LibraryId))
        .col(jsonb(VaultUnlockSessions::AuthContextJson))
        .col(text_null(VaultUnlockSessions::Acr))
        .col(text_array_default_empty(VaultUnlockSessions::Amr))
        .col(timestamptz(VaultUnlockSessions::ExpiresAt))
        .col(timestamptz(VaultUnlockSessions::CreatedAt))
        .col(timestamptz_null(VaultUnlockSessions::RevokedAt))
        .foreign_key(fk!(
            "fk_vault_unlock_sessions_user",
            VaultUnlockSessions::Table,
            VaultUnlockSessions::UserId,
            Users::Table,
            Users::Id,
        ))
        .foreign_key(fk!(
            "fk_vault_unlock_sessions_library",
            VaultUnlockSessions::Table,
            VaultUnlockSessions::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .to_owned()
}

fn audit_events_table() -> TableCreateStatement {
    Table::create()
        .table(AuditEvents::Table)
        .if_not_exists()
        .col(pk_uuid_v7(AuditEvents::Id))
        .col(uuid_null(AuditEvents::UserId))
        .col(uuid_null(AuditEvents::DeviceId))
        .col(uuid_null(AuditEvents::BrowserClientId))
        .col(uuid_null(AuditEvents::LibraryId))
        .col(text(AuditEvents::EventType))
        .col(jsonb(AuditEvents::PayloadJson))
        .col(timestamptz(AuditEvents::CreatedAt))
        .foreign_key(fk!(
            "fk_audit_events_user",
            AuditEvents::Table,
            AuditEvents::UserId,
            Users::Table,
            Users::Id,
        ))
        .foreign_key(fk!(
            "fk_audit_events_device",
            AuditEvents::Table,
            AuditEvents::DeviceId,
            Devices::Table,
            Devices::Id,
        ))
        .foreign_key(fk!(
            "fk_audit_events_browser_client",
            AuditEvents::Table,
            AuditEvents::BrowserClientId,
            BrowserClients::Table,
            BrowserClients::Id,
        ))
        .foreign_key(fk!(
            "fk_audit_events_library",
            AuditEvents::Table,
            AuditEvents::LibraryId,
            Libraries::Table,
            Libraries::Id,
        ))
        .to_owned()
}

async fn apply_rls(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    apply_rls_sql(manager, rls_statements()).await
}

fn rls_statements() -> Vec<String> {
    vec![
        owner_scoped_policy_sql(
            Users::Table,
            "users_owner_isolation",
            owner_match_condition(Users::Table, Users::Id),
        ),
        owner_scoped_policy_sql(
            AuthUsers::Table,
            "auth_users_owner_isolation",
            owner_match_condition(AuthUsers::Table, AuthUsers::UserId),
        ),
        owner_scoped_policy_sql(
            OidcAccountBindings::Table,
            "oidc_account_bindings_owner_isolation",
            owner_match_condition(OidcAccountBindings::Table, OidcAccountBindings::UserId),
        ),
        select_policy_sql(
            OidcAccountBindings::Table,
            "oidc_account_bindings_auth_lookup",
            oidc_account_binding_lookup_condition(),
        ),
        owner_scoped_policy_sql(
            Devices::Table,
            "devices_owner_isolation",
            owner_match_condition(Devices::Table, Devices::UserId),
        ),
        owner_scoped_policy_sql(
            BrowserClients::Table,
            "browser_clients_owner_isolation",
            browser_client_owner_condition(BrowserClients::Table, BrowserClients::DeviceId),
        ),
        owner_scoped_policy_sql(
            Libraries::Table,
            "libraries_owner_isolation",
            owner_match_condition(Libraries::Table, Libraries::OwnerUserId),
        ),
        owner_scoped_policy_sql(
            BookmarkNodes::Table,
            "bookmark_nodes_owner_isolation",
            library_owner_exists_condition(BookmarkNodes::Table, BookmarkNodes::LibraryId),
        ),
        owner_scoped_policy_sql(
            BookmarkMeta::Table,
            "bookmark_meta_owner_isolation",
            bookmark_meta_owner_condition(),
        ),
        owner_scoped_policy_sql(
            LibraryHeads::Table,
            "library_heads_owner_isolation",
            library_owner_exists_condition(LibraryHeads::Table, LibraryHeads::LibraryId),
        ),
        owner_scoped_policy_sql(
            NodeRevisions::Table,
            "node_revisions_owner_isolation",
            library_owner_exists_condition(NodeRevisions::Table, NodeRevisions::LibraryId),
        ),
        owner_scoped_policy_sql(
            SyncCursors::Table,
            "sync_cursors_owner_isolation",
            Condition::all()
                .add(library_owner_exists_condition(
                    SyncCursors::Table,
                    SyncCursors::LibraryId,
                ))
                .add(browser_client_owner_condition(
                    SyncCursors::Table,
                    SyncCursors::BrowserClientId,
                )),
        ),
        owner_scoped_policy_sql(
            NodeClientMappings::Table,
            "node_client_mappings_owner_isolation",
            Condition::all()
                .add(bookmark_node_owner_condition(
                    NodeClientMappings::Table,
                    NodeClientMappings::ServerNodeId,
                ))
                .add(browser_client_owner_condition(
                    NodeClientMappings::Table,
                    NodeClientMappings::BrowserClientId,
                )),
        ),
        owner_scoped_policy_sql(
            SyncConflicts::Table,
            "sync_conflicts_owner_isolation",
            Condition::all()
                .add(library_owner_exists_condition(
                    SyncConflicts::Table,
                    SyncConflicts::LibraryId,
                ))
                .add(browser_client_owner_condition(
                    SyncConflicts::Table,
                    SyncConflicts::BrowserClientId,
                )),
        ),
        owner_scoped_policy_sql(
            SyncProfiles::Table,
            "sync_profiles_owner_isolation",
            owner_match_condition(SyncProfiles::Table, SyncProfiles::UserId),
        ),
        owner_scoped_policy_sql(
            SyncProfileTargets::Table,
            "sync_profile_targets_owner_isolation",
            sync_profile_owner_condition(SyncProfileTargets::Table, SyncProfileTargets::ProfileId),
        ),
        owner_scoped_policy_sql(
            SyncProfileRules::Table,
            "sync_profile_rules_owner_isolation",
            sync_profile_owner_condition(SyncProfileRules::Table, SyncProfileRules::ProfileId),
        ),
        owner_scoped_policy_sql(
            SyncPreviews::Table,
            "sync_previews_owner_isolation",
            owner_match_condition(SyncPreviews::Table, SyncPreviews::UserId),
        ),
        owner_scoped_policy_sql(
            VaultUnlockSessions::Table,
            "vault_unlock_sessions_owner_isolation",
            Condition::all()
                .add(owner_match_condition(
                    VaultUnlockSessions::Table,
                    VaultUnlockSessions::UserId,
                ))
                .add(library_owner_exists_condition(
                    VaultUnlockSessions::Table,
                    VaultUnlockSessions::LibraryId,
                )),
        ),
        owner_scoped_policy_sql(
            AuditEvents::Table,
            "audit_events_owner_isolation",
            audit_events_owner_condition(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use sea_orm_migration::sea_query::PostgresQueryBuilder;

    use super::*;

    #[test]
    fn users_table_uses_uuidv7_and_omits_oidc_subject() {
        let sql = users_table().to_string(PostgresQueryBuilder);

        assert!(sql.contains("uuidv7()"));
        assert!(!sql.contains("oidc_subject"));
    }

    #[test]
    fn oidc_account_bindings_table_stores_subject() {
        let sql = oidc_account_bindings_table().to_string(PostgresQueryBuilder);

        assert!(sql.contains("oidc_subject"));
        assert!(sql.contains("oidc_identity_key"));
    }

    #[test]
    fn oidc_binding_unique_index_is_rendered() {
        let sql = unique_index(
            "idx_oidc_account_bindings_source_identity_key_unique",
            OidcAccountBindings::Table,
            &[
                OidcAccountBindings::OidcSource,
                OidcAccountBindings::OidcIdentityKey,
            ],
        )
        .to_string(PostgresQueryBuilder);

        assert!(sql.contains("CREATE UNIQUE INDEX"));
        assert!(sql.contains("oidc_account_bindings"));
        assert!(sql.contains("oidc_source"));
        assert!(sql.contains("oidc_identity_key"));
    }

    #[test]
    fn oidc_subject_lookup_index_is_rendered() {
        let sql = index(
            "idx_oidc_account_bindings_source_subject",
            OidcAccountBindings::Table,
            &[
                OidcAccountBindings::OidcSource,
                OidcAccountBindings::OidcSubject,
            ],
        )
        .to_string(PostgresQueryBuilder);

        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("oidc_source"));
        assert!(sql.contains("oidc_subject"));
    }

    #[test]
    fn libraries_rls_sql_uses_current_user_contract() {
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
    fn sync_cursor_composite_primary_key_is_rendered() {
        let sql = sync_cursors_table().to_string(PostgresQueryBuilder);

        assert!(sql.contains("pk_sync_cursors"));
        assert!(sql.contains("PRIMARY KEY (\"browser_client_id\", \"library_id\")"));
    }

    #[test]
    fn updated_at_trigger_table_list_covers_mutable_timestamp_tables() {
        assert_eq!(updated_at_tables()[0], "users");
        assert!(updated_at_tables().contains(&"bookmark_nodes"));
        assert!(updated_at_tables().contains(&"sync_profile_rules"));
        assert!(updated_at_tables().contains(&"sync_previews"));
    }

    #[test]
    fn sync_previews_table_uses_status_check_and_json_defaults() {
        let sql = sync_previews_table().to_string(PostgresQueryBuilder);

        assert!(sql.contains("sync_previews"));
        assert!(sql.contains("ck_sync_previews_status"));
        assert!(sql.contains("'[]'::jsonb"));
    }
}

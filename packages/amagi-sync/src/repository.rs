use amagi_db::entities::{
    browser_clients, devices, libraries, library_heads, node_client_mappings, node_revisions,
    sync_conflicts, sync_cursors, sync_previews, sync_profile_rules, sync_profiles,
};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    SyncError, SyncResult,
    model::{
        AcceptedLocalMutationView, BrowserClientRegistrationRequest, BrowserClientView,
        CursorSummaryView, DeviceRegistrationRequest, DeviceView, LocalMutationInput,
        NodeClientMappingView, PreviewSummaryView, ServerOpView, SyncConflictView, SyncLibraryView,
        SyncProfileRuleView, SyncProfileView,
    },
};

const DEFAULT_DEVICE_TRUST_LEVEL: &str = "trusted";
const DEFAULT_PROFILE_NAME: &str = "Default Manual Sync";

pub struct SyncRepository;

impl SyncRepository {
    pub async fn upsert_device(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        request: &DeviceRegistrationRequest,
    ) -> SyncResult<devices::Model> {
        let now = now();
        let device_id = match request.device_id.as_deref() {
            Some(device_id) => {
                Uuid::parse_str(device_id).map_err(|_| SyncError::InvalidRequest {
                    code: "device_id_invalid",
                    message: "device.deviceId must be a UUID".to_owned(),
                })?
            }
            None => Uuid::now_v7(),
        };

        let existing = devices::Entity::find_by_id(device_id)
            .filter(devices::Column::UserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load device for register",
            })?;

        match existing {
            Some(device) => {
                let mut active = device.into_active_model();
                active.device_name = Set(request.device_name.trim().to_owned());
                active.device_type = Set(request.device_type.trim().to_owned());
                active.platform = Set(request.platform.trim().to_owned());
                active.last_seen_at = Set(Some(now));
                active
                    .update(txn)
                    .await
                    .map_err(|_| SyncError::DatabaseQuery {
                        action: "update device for register",
                    })
            }
            None if request.device_id.is_some() => Err(SyncError::DeviceNotFound),
            None => devices::ActiveModel {
                id: Set(device_id),
                user_id: Set(owner_user_id),
                device_name: Set(request.device_name.trim().to_owned()),
                device_type: Set(request.device_type.trim().to_owned()),
                platform: Set(request.platform.trim().to_owned()),
                trust_level: Set(DEFAULT_DEVICE_TRUST_LEVEL.to_owned()),
                last_seen_at: Set(Some(now)),
                ..Default::default()
            }
            .insert(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "insert device for register",
            }),
        }
    }

    pub async fn upsert_browser_client(
        txn: &DatabaseTransaction,
        device_id: Uuid,
        request: &BrowserClientRegistrationRequest,
    ) -> SyncResult<browser_clients::Model> {
        let now = now();
        let existing = browser_clients::Entity::find()
            .filter(browser_clients::Column::DeviceId.eq(device_id))
            .filter(
                browser_clients::Column::ExtensionInstanceId
                    .eq(request.extension_instance_id.trim().to_owned()),
            )
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load browser client for register",
            })?;

        match existing {
            Some(client) => {
                let mut active = client.into_active_model();
                active.browser_family = Set(request.browser_family.trim().to_owned());
                active.browser_profile_name = Set(request
                    .browser_profile_name
                    .as_deref()
                    .map(str::trim)
                    .map(ToOwned::to_owned));
                active.capabilities_json = Set(request.capabilities.clone());
                active.last_seen_at = Set(Some(now));
                active
                    .update(txn)
                    .await
                    .map_err(|_| SyncError::DatabaseQuery {
                        action: "update browser client for register",
                    })
            }
            None => browser_clients::ActiveModel {
                id: Set(Uuid::now_v7()),
                device_id: Set(device_id),
                browser_family: Set(request.browser_family.trim().to_owned()),
                browser_profile_name: Set(request
                    .browser_profile_name
                    .as_deref()
                    .map(str::trim)
                    .map(ToOwned::to_owned)),
                extension_instance_id: Set(request.extension_instance_id.trim().to_owned()),
                capabilities_json: Set(request.capabilities.clone()),
                last_seen_at: Set(Some(now)),
                ..Default::default()
            }
            .insert(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "insert browser client for register",
            }),
        }
    }

    pub async fn find_browser_client_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        browser_client_id: Uuid,
    ) -> SyncResult<browser_clients::Model> {
        let client = browser_clients::Entity::find_by_id(browser_client_id)
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load browser client",
            })?
            .ok_or(SyncError::BrowserClientNotFound)?;

        let device = devices::Entity::find_by_id(client.device_id)
            .filter(devices::Column::UserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load browser client owner device",
            })?;

        if device.is_none() {
            return Err(SyncError::BrowserClientNotFound);
        }

        Ok(client)
    }

    pub async fn find_library_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        library_id: Uuid,
    ) -> SyncResult<libraries::Model> {
        libraries::Entity::find_by_id(library_id)
            .filter(libraries::Column::OwnerUserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load sync library",
            })?
            .ok_or(SyncError::LibraryNotFound)
    }

    pub async fn list_libraries_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
    ) -> SyncResult<Vec<libraries::Model>> {
        libraries::Entity::find()
            .filter(libraries::Column::OwnerUserId.eq(owner_user_id))
            .order_by_asc(libraries::Column::CreatedAt)
            .all(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "list sync libraries",
            })
    }

    pub async fn library_head_clock(
        txn: &DatabaseTransaction,
        library_id: Uuid,
    ) -> SyncResult<i64> {
        library_heads::Entity::find_by_id(library_id)
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load library head clock",
            })?
            .map(|head| head.current_revision_clock)
            .ok_or(SyncError::LibraryNotFound)
    }

    pub async fn ensure_default_profile(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
    ) -> SyncResult<sync_profiles::Model> {
        if let Some(profile) = sync_profiles::Entity::find()
            .filter(sync_profiles::Column::UserId.eq(owner_user_id))
            .filter(sync_profiles::Column::Mode.eq("manual"))
            .order_by_asc(sync_profiles::Column::CreatedAt)
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load default sync profile",
            })?
        {
            return Ok(profile);
        }

        let profile = sync_profiles::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(owner_user_id),
            name: Set(DEFAULT_PROFILE_NAME.to_owned()),
            mode: Set("manual".to_owned()),
            default_direction: Set("bidirectional".to_owned()),
            conflict_policy: Set("manual".to_owned()),
            enabled: Set(true),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| SyncError::DatabaseQuery {
            action: "insert default sync profile",
        })?;

        for (rule_order, action, matcher_value) in [
            (1, "include", "library_kind:normal"),
            (2, "exclude", "library_kind:vault"),
        ] {
            sync_profile_rules::ActiveModel {
                id: Set(Uuid::now_v7()),
                profile_id: Set(profile.id),
                rule_order: Set(rule_order),
                action: Set(action.to_owned()),
                matcher_type: Set("library_kind".to_owned()),
                matcher_value: Set(matcher_value.to_owned()),
                options_json: Set(json!({})),
                ..Default::default()
            }
            .insert(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "insert default sync profile rule",
            })?;
        }

        Ok(profile)
    }

    pub async fn list_profiles_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
    ) -> SyncResult<Vec<sync_profiles::Model>> {
        sync_profiles::Entity::find()
            .filter(sync_profiles::Column::UserId.eq(owner_user_id))
            .order_by_asc(sync_profiles::Column::CreatedAt)
            .all(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "list sync profiles",
            })
    }

    pub async fn find_profile_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        profile_id: Uuid,
    ) -> SyncResult<sync_profiles::Model> {
        sync_profiles::Entity::find_by_id(profile_id)
            .filter(sync_profiles::Column::UserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load sync profile",
            })?
            .ok_or(SyncError::ProfileNotFound)
    }

    pub async fn list_profile_rules(
        txn: &DatabaseTransaction,
        profile_id: Uuid,
    ) -> SyncResult<Vec<sync_profile_rules::Model>> {
        sync_profile_rules::Entity::find()
            .filter(sync_profile_rules::Column::ProfileId.eq(profile_id))
            .order_by_asc(sync_profile_rules::Column::RuleOrder)
            .all(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "list sync profile rules",
            })
    }

    pub async fn list_cursors_for_browser_client(
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
    ) -> SyncResult<Vec<sync_cursors::Model>> {
        sync_cursors::Entity::find()
            .filter(sync_cursors::Column::BrowserClientId.eq(browser_client_id))
            .order_by_asc(sync_cursors::Column::LibraryId)
            .all(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "list sync cursors",
            })
    }

    pub async fn list_server_ops(
        txn: &DatabaseTransaction,
        library_id: Uuid,
        from_clock: i64,
        limit: u64,
    ) -> SyncResult<Vec<node_revisions::Model>> {
        node_revisions::Entity::find()
            .filter(node_revisions::Column::LibraryId.eq(library_id))
            .filter(node_revisions::Column::LogicalClock.gt(from_clock))
            .order_by_asc(node_revisions::Column::LogicalClock)
            .limit(limit)
            .all(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "list sync server ops",
            })
    }

    pub async fn find_node_mapping_by_external_id(
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        client_external_id: &str,
    ) -> SyncResult<Option<node_client_mappings::Model>> {
        node_client_mappings::Entity::find()
            .filter(node_client_mappings::Column::BrowserClientId.eq(browser_client_id))
            .filter(node_client_mappings::Column::ClientExternalId.eq(client_external_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load node client mapping by external id",
            })
    }

    pub async fn upsert_cursor(
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        library_id: Uuid,
        applied_clock: i64,
        last_ack_rev_id: Option<Uuid>,
    ) -> SyncResult<sync_cursors::Model> {
        let now = now();
        let existing = sync_cursors::Entity::find_by_id((browser_client_id, library_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load sync cursor for ack",
            })?;

        match existing {
            Some(cursor) if applied_clock < cursor.last_applied_clock => Ok(cursor),
            Some(cursor) => {
                let mut active = cursor.into_active_model();
                active.last_applied_clock = Set(applied_clock);
                active.last_ack_rev_id = Set(last_ack_rev_id);
                active.last_sync_at = Set(Some(now));
                active
                    .update(txn)
                    .await
                    .map_err(|_| SyncError::DatabaseQuery {
                        action: "update sync cursor",
                    })
            }
            None => sync_cursors::ActiveModel {
                browser_client_id: Set(browser_client_id),
                library_id: Set(library_id),
                last_applied_clock: Set(applied_clock),
                last_ack_rev_id: Set(last_ack_rev_id),
                last_sync_at: Set(Some(now)),
                ..Default::default()
            }
            .insert(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "insert sync cursor",
            }),
        }
    }

    pub async fn insert_preview(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        browser_client_id: Uuid,
        library_id: Uuid,
        base_clock: i64,
        to_clock: i64,
        status: &str,
        request_hash: String,
        summary: &PreviewSummaryView,
        server_ops: &[ServerOpView],
        accepted_local_mutations: &[AcceptedLocalMutationView],
        conflicts: &[SyncConflictView],
    ) -> SyncResult<sync_previews::Model> {
        sync_previews::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(owner_user_id),
            browser_client_id: Set(browser_client_id),
            library_id: Set(library_id),
            base_clock: Set(base_clock),
            to_clock: Set(to_clock),
            status: Set(status.to_owned()),
            request_hash: Set(request_hash),
            summary_json: Set(serde_json::to_value(summary).expect("summary serializes")),
            server_ops_json: Set(serde_json::to_value(server_ops).expect("server ops serialize")),
            accepted_local_mutations_json: Set(serde_json::to_value(accepted_local_mutations)
                .expect("accepted local mutations serialize")),
            conflicts_json: Set(serde_json::to_value(conflicts).expect("conflicts serialize")),
            expires_at: Set(now() + Duration::minutes(10)),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| SyncError::DatabaseQuery {
            action: "insert sync preview",
        })
    }

    pub async fn find_preview_for_owner(
        txn: &DatabaseTransaction,
        owner_user_id: Uuid,
        preview_id: Uuid,
    ) -> SyncResult<sync_previews::Model> {
        sync_previews::Entity::find_by_id(preview_id)
            .filter(sync_previews::Column::UserId.eq(owner_user_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load sync preview",
            })?
            .ok_or(SyncError::PreviewNotFound)
    }

    pub async fn mark_preview_applied(
        txn: &DatabaseTransaction,
        preview: sync_previews::Model,
        apply_result: &Value,
    ) -> SyncResult<sync_previews::Model> {
        let mut active = preview.into_active_model();
        active.status = Set("applied".to_owned());
        active.applied_at = Set(Some(now()));
        let mut summary_json = active.summary_json.clone().unwrap();
        if let Some(object) = summary_json.as_object_mut() {
            object.insert("applyResult".to_owned(), apply_result.clone());
        }
        active.summary_json = Set(summary_json);
        active
            .update(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "mark sync preview applied",
            })
    }

    pub async fn mark_preview_expired(
        txn: &DatabaseTransaction,
        preview: sync_previews::Model,
    ) -> SyncResult<sync_previews::Model> {
        let mut active = preview.into_active_model();
        active.status = Set("expired".to_owned());
        active
            .update(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "mark sync preview expired",
            })
    }

    pub async fn insert_mapping(
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        server_node_id: Uuid,
        client_external_id: &str,
    ) -> SyncResult<node_client_mappings::Model> {
        if let Some(existing) = node_client_mappings::Entity::find()
            .filter(node_client_mappings::Column::BrowserClientId.eq(browser_client_id))
            .filter(node_client_mappings::Column::ClientExternalId.eq(client_external_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load mapping before insert",
            })?
        {
            if existing.server_node_id != server_node_id {
                return Err(SyncError::InvalidRequest {
                    code: "mapping_already_exists",
                    message: "clientExternalId is already mapped for this browser client"
                        .to_owned(),
                });
            }
            return Ok(existing);
        }

        node_client_mappings::ActiveModel {
            browser_client_id: Set(browser_client_id),
            server_node_id: Set(server_node_id),
            client_external_id: Set(client_external_id.to_owned()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| SyncError::DatabaseQuery {
            action: "insert node client mapping",
        })
    }

    pub async fn insert_conflict(
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        library_id: Uuid,
        conflict: &SyncConflictView,
    ) -> SyncResult<sync_conflicts::Model> {
        sync_conflicts::ActiveModel {
            id: Set(Uuid::now_v7()),
            browser_client_id: Set(browser_client_id),
            library_id: Set(library_id),
            conflict_type: Set(conflict.conflict_type.clone()),
            state: Set("open".to_owned()),
            summary: Set(conflict.summary.clone()),
            details_json: Set(conflict.details.clone()),
            ..Default::default()
        }
        .insert(txn)
        .await
        .map_err(|_| SyncError::DatabaseQuery {
            action: "insert sync conflict",
        })
    }
}

pub fn device_view(model: devices::Model) -> DeviceView {
    DeviceView {
        id: model.id.to_string(),
        device_name: model.device_name,
        device_type: model.device_type,
        platform: model.platform,
        trust_level: model.trust_level,
        last_seen_at: model.last_seen_at.map(|value| value.to_rfc3339()),
    }
}

pub fn browser_client_view(model: browser_clients::Model) -> BrowserClientView {
    BrowserClientView {
        id: model.id.to_string(),
        device_id: model.device_id.to_string(),
        browser_family: model.browser_family,
        browser_profile_name: model.browser_profile_name,
        extension_instance_id: model.extension_instance_id,
        capabilities: model.capabilities_json,
        last_seen_at: model.last_seen_at.map(|value| value.to_rfc3339()),
    }
}

pub fn profile_view(
    model: sync_profiles::Model,
    rules: Vec<sync_profile_rules::Model>,
) -> SyncProfileView {
    SyncProfileView {
        id: model.id.to_string(),
        name: model.name,
        mode: model.mode,
        default_direction: model.default_direction,
        conflict_policy: model.conflict_policy,
        enabled: model.enabled,
        rules: rules
            .into_iter()
            .map(|rule| SyncProfileRuleView {
                id: rule.id.to_string(),
                rule_order: rule.rule_order,
                action: rule.action,
                matcher_type: rule.matcher_type,
                matcher_value: rule.matcher_value,
            })
            .collect(),
    }
}

pub fn cursor_view(model: sync_cursors::Model) -> CursorSummaryView {
    CursorSummaryView {
        browser_client_id: model.browser_client_id.to_string(),
        library_id: model.library_id.to_string(),
        last_applied_clock: model.last_applied_clock,
        last_ack_rev_id: model.last_ack_rev_id.map(|value| value.to_string()),
        last_sync_at: model.last_sync_at.map(|value| value.to_rfc3339()),
    }
}

pub fn library_view(model: libraries::Model, current_revision_clock: i64) -> SyncLibraryView {
    SyncLibraryView {
        id: model.id.to_string(),
        name: model.name,
        projection: if model.kind == "vault" {
            "excluded".to_owned()
        } else {
            "include".to_owned()
        },
        kind: model.kind,
        current_revision_clock,
    }
}

pub fn server_op_view(model: node_revisions::Model) -> ServerOpView {
    ServerOpView {
        rev_id: model.rev_id.to_string(),
        node_id: model.node_id.to_string(),
        op_type: model.op_type,
        logical_clock: model.logical_clock,
        payload: model.payload_json,
        created_at: model.created_at.to_rfc3339(),
    }
}

pub fn mapping_view(model: node_client_mappings::Model) -> NodeClientMappingView {
    NodeClientMappingView {
        browser_client_id: model.browser_client_id.to_string(),
        server_node_id: model.server_node_id.to_string(),
        client_external_id: model.client_external_id,
    }
}

pub fn hash_json<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(value).expect("value serializes"));
    format!("{:x}", hasher.finalize())
}

pub fn now() -> chrono::DateTime<chrono::FixedOffset> {
    Utc::now().fixed_offset()
}

pub fn parse_uuid(value: &str, code: &'static str, field_name: &str) -> SyncResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| SyncError::InvalidRequest {
        code,
        message: format!("{field_name} must be a UUID"),
    })
}

pub fn validate_local_mutations(local_mutations: &[LocalMutationInput]) -> SyncResult<()> {
    let mut seen = std::collections::BTreeSet::new();
    for mutation in local_mutations {
        if mutation.client_mutation_id.trim().is_empty() {
            return Err(SyncError::InvalidRequest {
                code: "client_mutation_id_required",
                message: "localMutations[].clientMutationId must be non-empty".to_owned(),
            });
        }
        if !seen.insert(mutation.client_mutation_id.clone()) {
            return Err(SyncError::InvalidRequest {
                code: "client_mutation_id_duplicate",
                message: "localMutations[].clientMutationId must be unique within one preview"
                    .to_owned(),
            });
        }
    }

    Ok(())
}

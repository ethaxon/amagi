use amagi_bookmarks::{
    CreateNodeRequest, LibraryKind, MoveNodeRequest, NodeType, UpdateNodeRequest,
};
use amagi_db::{CurrentUserId, DatabaseService, entities::bookmark_nodes, set_current_user_id};
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    SyncError, SyncResult,
    model::{
        AcceptedLocalMutationView, CreateSyncProfileRequest, CreateSyncProfileRuleRequest,
        CreateSyncProfileTargetRequest, CursorAckRequest, CursorAckResponse, FeedRequest,
        FeedResponse, LocalMutationInput, PreviewSummaryView, RegisterClientRequest,
        RegisterClientResponse, ServerOpView, SyncApplyRequest, SyncApplyResponse,
        SyncConflictView, SyncPreviewRequest, SyncPreviewResponse, SyncProfileDetailView,
        SyncSessionStartRequest, SyncSessionStartResponse, UpdateSyncProfileRequest,
        UpdateSyncProfileRuleRequest,
    },
    repository::{
        InsertPreviewParams, SyncRepository, browser_client_view, cursor_view, device_view,
        hash_json, library_view, mapping_view, now, parse_uuid, profile_detail_view, profile_view,
        server_op_view, validate_local_mutations,
    },
};

#[derive(Debug, Clone)]
pub struct SyncService {
    database: DatabaseService,
    bookmarks: amagi_bookmarks::BookmarkService,
}

impl SyncService {
    pub fn new(database: DatabaseService, bookmarks: amagi_bookmarks::BookmarkService) -> Self {
        Self {
            database,
            bookmarks,
        }
    }

    pub async fn register_client(
        &self,
        owner_user_id: Uuid,
        request: &RegisterClientRequest,
    ) -> SyncResult<RegisterClientResponse> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let device =
                SyncRepository::upsert_device(&txn, owner_user_id, &request.device).await?;
            let browser_client =
                SyncRepository::upsert_browser_client(&txn, device.id, &request.browser_client)
                    .await?;
            let default_profile =
                SyncRepository::ensure_default_profile(&txn, owner_user_id).await?;
            let default_rules =
                SyncRepository::list_profile_rules(&txn, default_profile.id).await?;
            let cursors =
                SyncRepository::list_cursors_for_browser_client(&txn, browser_client.id).await?;

            Ok(RegisterClientResponse {
                device: device_view(device),
                browser_client: browser_client_view(browser_client),
                default_profile: profile_view(default_profile, default_rules),
                cursor_summaries: cursors.into_iter().map(cursor_view).collect(),
            })
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn start_session(
        &self,
        owner_user_id: Uuid,
        request: &SyncSessionStartRequest,
    ) -> SyncResult<SyncSessionStartResponse> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let browser_client_id = parse_uuid(
                &request.browser_client_id,
                "browser_client_id_invalid",
                "browserClientId",
            )?;
            let browser_client = SyncRepository::find_browser_client_for_owner(
                &txn,
                owner_user_id,
                browser_client_id,
            )
            .await?;
            let default_profile =
                SyncRepository::ensure_default_profile(&txn, owner_user_id).await?;
            let selected_profile = match request.preferred_profile_id.as_deref() {
                Some(profile_id) => {
                    SyncRepository::find_enabled_profile_for_owner(
                        &txn,
                        owner_user_id,
                        parse_uuid(profile_id, "profile_id_invalid", "preferredProfileId")?,
                    )
                    .await?
                }
                None => default_profile.clone(),
            };
            let available_profiles =
                SyncRepository::list_enabled_profiles_for_owner(&txn, owner_user_id).await?;
            let libraries = SyncRepository::list_libraries_for_owner(&txn, owner_user_id).await?;
            let cursors =
                SyncRepository::list_cursors_for_browser_client(&txn, browser_client_id).await?;

            let mut profile_views = Vec::with_capacity(available_profiles.len());
            for profile in available_profiles {
                let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
                profile_views.push(profile_view(profile, rules));
            }

            let selected_rules =
                SyncRepository::list_profile_rules(&txn, selected_profile.id).await?;
            let mut library_views = Vec::with_capacity(libraries.len());
            for library in libraries {
                let head_clock = SyncRepository::library_head_clock(&txn, library.id).await?;
                library_views.push(library_view(library, head_clock));
            }

            Ok(SyncSessionStartResponse {
                browser_client: browser_client_view(browser_client),
                selected_profile: profile_view(selected_profile, selected_rules),
                available_profiles: profile_views,
                libraries: library_views,
                cursors: cursors.into_iter().map(cursor_view).collect(),
                server_time: Utc::now().to_rfc3339(),
            })
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn list_profile_details(
        &self,
        owner_user_id: Uuid,
    ) -> SyncResult<Vec<SyncProfileDetailView>> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            SyncRepository::ensure_default_profile(&txn, owner_user_id).await?;
            let profiles = SyncRepository::list_profiles_for_owner(&txn, owner_user_id).await?;
            let mut views = Vec::with_capacity(profiles.len());
            for profile in profiles {
                let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
                let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
                views.push(profile_detail_view(profile, rules, targets));
            }
            Ok(views)
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn create_profile(
        &self,
        owner_user_id: Uuid,
        request: &CreateSyncProfileRequest,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            validate_profile_create_request(request)?;
            let profile = SyncRepository::create_profile(&txn, owner_user_id, request).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn update_profile(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        request: &UpdateSyncProfileRequest,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            validate_profile_update_request(request)?;
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            if matches!(request.enabled, Some(false))
                && profile.mode == "manual"
                && profile.enabled
                && SyncRepository::count_enabled_manual_profiles(&txn, owner_user_id).await? <= 1
            {
                return Err(SyncError::LastEnabledManualProfile);
            }
            let profile = SyncRepository::update_profile(&txn, profile, request).await?;
            if profile.mode == "manual" && profile.enabled {
                SyncRepository::ensure_default_profile(&txn, owner_user_id).await?;
            }
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn create_profile_target(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        request: &CreateSyncProfileTargetRequest,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let normalized_request = normalize_target_request(request)?;
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            if let Some(device_id) = normalized_request.device_id.as_deref() {
                SyncRepository::find_device_for_owner(
                    &txn,
                    owner_user_id,
                    parse_uuid(device_id, "device_id_invalid", "deviceId")?,
                )
                .await?;
            }
            if let Some(browser_client_id) = normalized_request.browser_client_id.as_deref() {
                SyncRepository::find_browser_client_for_owner(
                    &txn,
                    owner_user_id,
                    parse_uuid(
                        browser_client_id,
                        "browser_client_id_invalid",
                        "browserClientId",
                    )?,
                )
                .await?;
            }
            SyncRepository::create_profile_target(&txn, profile.id, &normalized_request).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn delete_profile_target(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        target_id: Uuid,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            let target = SyncRepository::find_profile_target(&txn, profile.id, target_id).await?;
            SyncRepository::delete_profile_target(&txn, target).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn create_profile_rule(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        request: &CreateSyncProfileRuleRequest,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let normalized_request = validate_profile_rule_create_request(request)?;
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            SyncRepository::create_profile_rule(&txn, profile.id, &normalized_request).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn update_profile_rule(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        rule_id: Uuid,
        request: &UpdateSyncProfileRuleRequest,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            let normalized_request = validate_profile_rule_update_request(request)?;
            let rule = SyncRepository::find_profile_rule(&txn, profile.id, rule_id).await?;
            SyncRepository::update_profile_rule(&txn, rule, &normalized_request).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn delete_profile_rule(
        &self,
        owner_user_id: Uuid,
        profile_id: Uuid,
        rule_id: Uuid,
    ) -> SyncResult<SyncProfileDetailView> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let profile =
                SyncRepository::find_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            let rule = SyncRepository::find_profile_rule(&txn, profile.id, rule_id).await?;
            SyncRepository::delete_profile_rule(&txn, rule).await?;
            let rules = SyncRepository::list_profile_rules(&txn, profile.id).await?;
            let targets = SyncRepository::list_profile_targets(&txn, profile.id).await?;
            Ok(profile_detail_view(profile, rules, targets))
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn feed(
        &self,
        owner_user_id: Uuid,
        request: &FeedRequest,
    ) -> SyncResult<FeedResponse> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let browser_client_id = parse_uuid(
                &request.browser_client_id,
                "browser_client_id_invalid",
                "browserClientId",
            )?;
            let library_id = parse_uuid(&request.library_id, "library_id_invalid", "libraryId")?;
            SyncRepository::find_browser_client_for_owner(&txn, owner_user_id, browser_client_id)
                .await?;
            let library =
                SyncRepository::find_library_for_owner(&txn, owner_user_id, library_id).await?;
            if library.kind == LibraryKind::Vault.as_str() {
                return Err(SyncError::VaultSyncNotSupported);
            }
            if let Some(profile_id) = request.profile_id.as_deref() {
                let profile_id = parse_uuid(profile_id, "profile_id_invalid", "profileId")?;
                SyncRepository::find_enabled_profile_for_owner(&txn, owner_user_id, profile_id)
                    .await?;
            }

            let current_clock = SyncRepository::library_head_clock(&txn, library_id).await?;
            let server_ops = SyncRepository::list_server_ops(
                &txn,
                library_id,
                request.from_clock,
                request.limit.unwrap_or(100).clamp(1, 500),
            )
            .await?;
            let to_clock = server_ops
                .last()
                .map(|op| op.logical_clock)
                .unwrap_or(current_clock);
            let next_cursor =
                if server_ops.len() as u64 == request.limit.unwrap_or(100).clamp(1, 500) {
                    Some(to_clock)
                } else {
                    None
                };

            Ok(FeedResponse {
                browser_client_id: browser_client_id.to_string(),
                library_id: library_id.to_string(),
                from_clock: request.from_clock,
                to_clock,
                current_clock,
                server_ops: server_ops.into_iter().map(server_op_view).collect(),
                next_cursor,
            })
        }
        .await;

        finish_read_txn(txn, result).await
    }

    pub async fn preview(
        &self,
        owner_user_id: Uuid,
        request: &SyncPreviewRequest,
    ) -> SyncResult<SyncPreviewResponse> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            validate_local_mutations(&request.local_mutations)?;
            let browser_client_id = parse_uuid(
                &request.browser_client_id,
                "browser_client_id_invalid",
                "browserClientId",
            )?;
            let profile_id = parse_uuid(&request.profile_id, "profile_id_invalid", "profileId")?;
            let library_id = parse_uuid(&request.library_id, "library_id_invalid", "libraryId")?;
            SyncRepository::find_browser_client_for_owner(&txn, owner_user_id, browser_client_id)
                .await?;
            SyncRepository::find_enabled_profile_for_owner(&txn, owner_user_id, profile_id).await?;
            let library =
                SyncRepository::find_library_for_owner(&txn, owner_user_id, library_id).await?;

            let current_clock = SyncRepository::library_head_clock(&txn, library_id).await?;
            let server_ops =
                SyncRepository::list_server_ops(&txn, library_id, request.base_clock, 500)
                    .await?
                    .into_iter()
                    .map(server_op_view)
                    .collect::<Vec<_>>();

            let mut conflicts = Vec::new();
            let mut accepted_local_mutations = Vec::new();
            if library.kind == LibraryKind::Vault.as_str() {
                conflicts.push(conflict(
                    "unsupported_vault_sync",
                    "vault library is excluded from Iter7 sync preview",
                    json!({ "libraryId": library_id }),
                ));
            } else if request.base_clock < current_clock && !request.local_mutations.is_empty() {
                conflicts.push(conflict(
                    "stale_base_clock",
                    "local mutations require a fresh preview after pulling newer server revisions",
                    json!({
                        "baseClock": request.base_clock,
                        "currentClock": current_clock,
                    }),
                ));
            } else {
                for mutation in &request.local_mutations {
                    match self
                        .accept_local_mutation(&txn, browser_client_id, library_id, mutation)
                        .await
                    {
                        Ok(accepted) => accepted_local_mutations.push(accepted),
                        Err(
                            error @ (SyncError::InvalidRequest { .. } | SyncError::LibraryNotFound),
                        ) => return Err(error),
                        Err(SyncError::VaultSyncNotSupported) => {
                            conflicts.push(conflict(
                                "unsupported_vault_sync",
                                "vault library is excluded from Iter7 sync preview",
                                json!({ "libraryId": library_id }),
                            ));
                        }
                        Err(SyncError::PreviewStale) => {
                            conflicts.push(conflict(
                                "mapping_missing",
                                "clientExternalId could not be resolved to a server mapping",
                                json!({ "clientMutationId": mutation.client_mutation_id }),
                            ));
                        }
                        Err(SyncError::BrowserClientNotFound) => {
                            return Err(SyncError::BrowserClientNotFound);
                        }
                        Err(error) => return Err(error),
                    }
                }
            }

            let summary = PreviewSummaryView {
                server_to_local: server_ops.len(),
                local_to_server_accepted: accepted_local_mutations.len(),
                conflicts: conflicts.len(),
            };
            let status = if conflicts.is_empty() {
                "pending"
            } else {
                "conflicted"
            };
            let preview = SyncRepository::insert_preview(
                &txn,
                InsertPreviewParams {
                    owner_user_id,
                    browser_client_id,
                    library_id,
                    base_clock: request.base_clock,
                    to_clock: current_clock,
                    status,
                    request_hash: hash_json(request),
                    summary: &summary,
                    server_ops: &server_ops,
                    accepted_local_mutations: &accepted_local_mutations,
                    conflicts: &conflicts,
                },
            )
            .await?;

            for conflict in &conflicts {
                SyncRepository::insert_conflict(&txn, browser_client_id, library_id, conflict)
                    .await?;
            }

            Ok(SyncPreviewResponse {
                preview_id: preview.id.to_string(),
                expires_at: preview.expires_at.to_rfc3339(),
                summary,
                server_ops,
                accepted_local_mutations,
                conflicts,
            })
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn apply(
        &self,
        owner_user_id: Uuid,
        request: &SyncApplyRequest,
    ) -> SyncResult<SyncApplyResponse> {
        if !request.confirm {
            return Err(SyncError::ConfirmationRequired);
        }

        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let preview_id = parse_uuid(&request.preview_id, "preview_id_invalid", "previewId")?;
            let preview =
                SyncRepository::find_preview_for_owner(&txn, owner_user_id, preview_id).await?;
            let browser_client = SyncRepository::find_browser_client_for_owner(
                &txn,
                owner_user_id,
                preview.browser_client_id,
            )
            .await?;
            let library =
                SyncRepository::find_library_for_owner(&txn, owner_user_id, preview.library_id)
                    .await?;

            if library.kind == LibraryKind::Vault.as_str() {
                return Err(SyncError::VaultSyncNotSupported);
            }
            if preview.status == "applied" {
                return parse_apply_result(preview.summary_json.clone());
            }
            if preview.status != "pending" {
                return Err(SyncError::PreviewStale);
            }
            if preview.expires_at < now() {
                let _ = SyncRepository::mark_preview_expired(&txn, preview).await?;
                return Err(SyncError::PreviewExpired);
            }

            let current_clock =
                SyncRepository::library_head_clock(&txn, preview.library_id).await?;
            if current_clock != preview.to_clock {
                return Err(SyncError::PreviewStale);
            }

            let server_ops_to_apply_locally: Vec<ServerOpView> =
                serde_json::from_value(preview.server_ops_json.clone()).map_err(|_| {
                    SyncError::DatabaseQuery {
                        action: "deserialize sync preview server ops",
                    }
                })?;
            let accepted_local_mutations: Vec<AcceptedLocalMutationView> = serde_json::from_value(
                preview.accepted_local_mutations_json.clone(),
            )
            .map_err(|_| SyncError::DatabaseQuery {
                action: "deserialize sync preview accepted local mutations",
            })?;

            let bookmark_txn = self.bookmarks.bind_txn(&txn, owner_user_id);
            let mut created_mappings = Vec::new();
            for mutation in accepted_local_mutations {
                match mutation.op.as_str() {
                    "create" => {
                        let client_external_id = mutation.client_external_id.as_deref().ok_or(
                            SyncError::InvalidRequest {
                                code: "client_external_id_required",
                                message: "accepted create mutation must carry clientExternalId"
                                    .to_owned(),
                            },
                        )?;
                        if SyncRepository::find_node_mapping_by_external_id(
                            &txn,
                            browser_client.id,
                            client_external_id,
                        )
                        .await?
                        .is_some()
                        {
                            return Err(SyncError::InvalidRequest {
                                code: "mapping_already_exists",
                                message: "clientExternalId is already mapped for this browser \
                                          client"
                                    .to_owned(),
                            });
                        }
                        let parent_id = parse_uuid(
                            mutation
                                .parent_server_node_id
                                .as_deref()
                                .expect("create parent exists"),
                            "parent_server_node_id_invalid",
                            "parentServerNodeId",
                        )?;
                        self.ensure_live_folder_in_library(
                            &txn,
                            parent_id,
                            preview.library_id,
                            "load parent node for sync apply create",
                        )
                        .await?;
                        let node_type = match mutation.node_type.as_deref() {
                            Some("folder") => NodeType::Folder,
                            Some("bookmark") => NodeType::Bookmark,
                            Some("separator") => NodeType::Separator,
                            _ => {
                                return Err(SyncError::InvalidRequest {
                                    code: "node_type_invalid",
                                    message: "accepted create mutation must carry a valid nodeType"
                                        .to_owned(),
                                });
                            }
                        };
                        let created = bookmark_txn
                            .create_node(
                                preview.library_id,
                                &CreateNodeRequest {
                                    node_type,
                                    parent_id: Some(parent_id),
                                    title: mutation.title.unwrap_or_default(),
                                    url: mutation.url,
                                    sort_key: mutation.sort_key,
                                },
                            )
                            .await
                            .map_err(map_bookmark_error)?;

                        let mapping = SyncRepository::insert_mapping(
                            &txn,
                            browser_client.id,
                            Uuid::parse_str(&created.id).expect("created node id is a UUID"),
                            client_external_id,
                        )
                        .await?;
                        created_mappings.push(mapping_view(mapping));
                    }
                    "update" => {
                        let node_id = parse_uuid(
                            mutation
                                .server_node_id
                                .as_deref()
                                .expect("update node id exists"),
                            "server_node_id_invalid",
                            "serverNodeId",
                        )?;
                        self.ensure_node_in_library(
                            &txn,
                            node_id,
                            preview.library_id,
                            "load node for sync apply update",
                        )
                        .await?;
                        bookmark_txn
                            .update_node(
                                node_id,
                                &UpdateNodeRequest {
                                    title: mutation.title,
                                    url: mutation.url,
                                },
                            )
                            .await
                            .map_err(map_bookmark_error)?;
                    }
                    "move" => {
                        let node_id = parse_uuid(
                            mutation
                                .server_node_id
                                .as_deref()
                                .expect("move node id exists"),
                            "server_node_id_invalid",
                            "serverNodeId",
                        )?;
                        self.ensure_node_in_library(
                            &txn,
                            node_id,
                            preview.library_id,
                            "load node for sync apply move",
                        )
                        .await?;
                        let parent_id = parse_uuid(
                            mutation
                                .parent_server_node_id
                                .as_deref()
                                .expect("move parent exists"),
                            "parent_server_node_id_invalid",
                            "parentServerNodeId",
                        )?;
                        self.ensure_live_folder_in_library(
                            &txn,
                            parent_id,
                            preview.library_id,
                            "load parent node for sync apply move",
                        )
                        .await?;
                        bookmark_txn
                            .move_node(
                                node_id,
                                &MoveNodeRequest {
                                    parent_id,
                                    sort_key: mutation.sort_key,
                                },
                            )
                            .await
                            .map_err(map_bookmark_error)?;
                    }
                    "delete" => {
                        let node_id = parse_uuid(
                            mutation
                                .server_node_id
                                .as_deref()
                                .expect("delete node id exists"),
                            "server_node_id_invalid",
                            "serverNodeId",
                        )?;
                        self.ensure_node_in_library(
                            &txn,
                            node_id,
                            preview.library_id,
                            "load node for sync apply delete",
                        )
                        .await?;
                        bookmark_txn
                            .delete_node(node_id)
                            .await
                            .map_err(map_bookmark_error)?;
                    }
                    "restore" => {
                        let node_id = parse_uuid(
                            mutation
                                .server_node_id
                                .as_deref()
                                .expect("restore node id exists"),
                            "server_node_id_invalid",
                            "serverNodeId",
                        )?;
                        self.ensure_node_in_library(
                            &txn,
                            node_id,
                            preview.library_id,
                            "load node for sync apply restore",
                        )
                        .await?;
                        bookmark_txn
                            .restore_node(node_id)
                            .await
                            .map_err(map_bookmark_error)?;
                    }
                    _ => {
                        return Err(SyncError::InvalidRequest {
                            code: "local_mutation_op_invalid",
                            message: "preview contains an unsupported mutation op".to_owned(),
                        });
                    }
                }
            }

            let new_clock = SyncRepository::library_head_clock(&txn, preview.library_id).await?;
            let response = SyncApplyResponse {
                applied: true,
                new_clock,
                server_ops_to_apply_locally,
                created_mappings,
                conflicts: Vec::new(),
            };

            let apply_result = serde_json::to_value(&response).expect("apply response serializes");
            let _ = SyncRepository::mark_preview_applied(&txn, preview, &apply_result).await?;
            Ok(response)
        }
        .await;

        finish_write_txn(txn, result).await
    }

    pub async fn ack_cursor(
        &self,
        owner_user_id: Uuid,
        request: &CursorAckRequest,
    ) -> SyncResult<CursorAckResponse> {
        let txn = self.begin_owner_txn(owner_user_id).await?;
        let result = async {
            let browser_client_id = parse_uuid(
                &request.browser_client_id,
                "browser_client_id_invalid",
                "browserClientId",
            )?;
            let library_id = parse_uuid(&request.library_id, "library_id_invalid", "libraryId")?;
            SyncRepository::find_browser_client_for_owner(&txn, owner_user_id, browser_client_id)
                .await?;
            SyncRepository::find_library_for_owner(&txn, owner_user_id, library_id).await?;
            let current_clock = SyncRepository::library_head_clock(&txn, library_id).await?;
            if request.applied_clock > current_clock {
                return Err(SyncError::InvalidRequest {
                    code: "applied_clock_ahead_of_head",
                    message: "appliedClock cannot exceed the current library head clock".to_owned(),
                });
            }
            let cursor = SyncRepository::upsert_cursor(
                &txn,
                browser_client_id,
                library_id,
                request.applied_clock,
                request
                    .last_ack_rev_id
                    .as_deref()
                    .map(|value| parse_uuid(value, "last_ack_rev_id_invalid", "lastAckRevId"))
                    .transpose()?,
            )
            .await?;
            Ok(CursorAckResponse {
                cursor: cursor_view(cursor),
            })
        }
        .await;

        finish_write_txn(txn, result).await
    }

    async fn begin_owner_txn(&self, owner_user_id: Uuid) -> SyncResult<DatabaseTransaction> {
        let runtime = self
            .database
            .runtime()
            .ok_or(SyncError::DatabaseUnavailable)?;
        let txn = runtime
            .connection()
            .begin()
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "start sync transaction",
            })?;
        set_current_user_id(&txn, CurrentUserId::new(owner_user_id))
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "set sync current user",
            })?;
        Ok(txn)
    }

    async fn accept_local_mutation(
        &self,
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        library_id: Uuid,
        mutation: &LocalMutationInput,
    ) -> SyncResult<AcceptedLocalMutationView> {
        match mutation.op.as_str() {
            "create" => {
                let parent_server_node_id = self
                    .resolve_parent_node_id(
                        txn,
                        browser_client_id,
                        mutation.parent_server_node_id.as_deref(),
                        mutation.parent_client_external_id.as_deref(),
                        library_id,
                    )
                    .await?;
                let node_type = mutation
                    .node_type
                    .clone()
                    .ok_or(SyncError::InvalidRequest {
                        code: "node_type_required",
                        message: "create mutation requires nodeType".to_owned(),
                    })?;
                let parsed_type = match node_type.as_str() {
                    "folder" => NodeType::Folder,
                    "bookmark" => NodeType::Bookmark,
                    "separator" => NodeType::Separator,
                    _ => {
                        return Err(SyncError::InvalidRequest {
                            code: "node_type_invalid",
                            message: "create mutation nodeType is invalid".to_owned(),
                        });
                    }
                };
                parsed_type
                    .validate_url(mutation.url.as_deref())
                    .map_err(map_bookmark_error)?;

                if mutation
                    .client_external_id
                    .as_deref()
                    .is_none_or(str::is_empty)
                {
                    return Err(SyncError::InvalidRequest {
                        code: "client_external_id_required",
                        message: "create mutation requires clientExternalId".to_owned(),
                    });
                }
                if SyncRepository::find_node_mapping_by_external_id(
                    txn,
                    browser_client_id,
                    mutation
                        .client_external_id
                        .as_deref()
                        .expect("client external id exists after validation"),
                )
                .await?
                .is_some()
                {
                    return Err(SyncError::InvalidRequest {
                        code: "mapping_already_exists",
                        message: "clientExternalId is already mapped for this browser client"
                            .to_owned(),
                    });
                }

                Ok(AcceptedLocalMutationView {
                    client_mutation_id: mutation.client_mutation_id.clone(),
                    op: mutation.op.clone(),
                    server_node_id: None,
                    client_external_id: mutation.client_external_id.clone(),
                    parent_server_node_id: Some(parent_server_node_id.to_string()),
                    node_type: Some(node_type),
                    title: Some(mutation.title.clone().unwrap_or_default()),
                    url: mutation.url.clone(),
                    sort_key: mutation.sort_key.clone(),
                })
            }
            "update" | "move" | "delete" | "restore" => {
                let server_node_id = self
                    .resolve_node_id(
                        txn,
                        browser_client_id,
                        mutation.server_node_id.as_deref(),
                        mutation.client_external_id.as_deref(),
                    )
                    .await?;
                let node = self
                    .ensure_node_in_library(
                        txn,
                        server_node_id,
                        library_id,
                        "load node for preview validation",
                    )
                    .await?;
                let parent_server_node_id = if mutation.op == "move" {
                    Some(
                        self.resolve_parent_node_id(
                            txn,
                            browser_client_id,
                            mutation.parent_server_node_id.as_deref(),
                            mutation.parent_client_external_id.as_deref(),
                            library_id,
                        )
                        .await?
                        .to_string(),
                    )
                } else {
                    None
                };

                if mutation.op == "update" {
                    let node_type =
                        NodeType::from_db(&node.node_type).map_err(map_bookmark_error)?;
                    if mutation.url.is_some() {
                        node_type
                            .validate_url(mutation.url.as_deref())
                            .map_err(map_bookmark_error)?;
                    }
                }

                Ok(AcceptedLocalMutationView {
                    client_mutation_id: mutation.client_mutation_id.clone(),
                    op: mutation.op.clone(),
                    server_node_id: Some(server_node_id.to_string()),
                    client_external_id: mutation.client_external_id.clone(),
                    parent_server_node_id,
                    node_type: mutation.node_type.clone(),
                    title: mutation.title.clone(),
                    url: mutation.url.clone(),
                    sort_key: mutation.sort_key.clone(),
                })
            }
            _ => Err(SyncError::InvalidRequest {
                code: "local_mutation_op_invalid",
                message: "localMutations[].op is invalid".to_owned(),
            }),
        }
    }

    async fn resolve_parent_node_id(
        &self,
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        parent_server_node_id: Option<&str>,
        parent_client_external_id: Option<&str>,
        library_id: Uuid,
    ) -> SyncResult<Uuid> {
        let parent_id = self
            .resolve_node_id(
                txn,
                browser_client_id,
                parent_server_node_id,
                parent_client_external_id,
            )
            .await?;
        let parent = bookmark_nodes::Entity::find_by_id(parent_id)
            .filter(bookmark_nodes::Column::LibraryId.eq(library_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery {
                action: "load parent node for preview",
            })?
            .ok_or(SyncError::InvalidRequest {
                code: "invalid_parent",
                message: "parent node does not belong to the selected library".to_owned(),
            })?;
        if parent.node_type != "folder" || parent.is_deleted {
            return Err(SyncError::InvalidRequest {
                code: "invalid_parent",
                message: "parent node must resolve to a live folder".to_owned(),
            });
        }
        Ok(parent_id)
    }

    async fn resolve_node_id(
        &self,
        txn: &DatabaseTransaction,
        browser_client_id: Uuid,
        server_node_id: Option<&str>,
        client_external_id: Option<&str>,
    ) -> SyncResult<Uuid> {
        let server_node_id = server_node_id
            .map(|value| parse_uuid(value, "server_node_id_invalid", "serverNodeId"))
            .transpose()?;
        let mapped_node_id = match client_external_id.filter(|value| !value.trim().is_empty()) {
            Some(client_external_id) => SyncRepository::find_node_mapping_by_external_id(
                txn,
                browser_client_id,
                client_external_id,
            )
            .await?
            .map(|mapping| mapping.server_node_id),
            None => None,
        };

        match (server_node_id, mapped_node_id) {
            (Some(server_node_id), Some(mapped_node_id)) if server_node_id != mapped_node_id => {
                Err(SyncError::InvalidRequest {
                    code: "node_target_mismatch",
                    message: "serverNodeId and clientExternalId resolved to different nodes"
                        .to_owned(),
                })
            }
            (Some(server_node_id), _) => Ok(server_node_id),
            (None, Some(mapped_node_id)) => Ok(mapped_node_id),
            (None, None) => Err(SyncError::PreviewStale),
        }
    }

    async fn ensure_node_in_library(
        &self,
        txn: &DatabaseTransaction,
        node_id: Uuid,
        library_id: Uuid,
        action: &'static str,
    ) -> SyncResult<bookmark_nodes::Model> {
        bookmark_nodes::Entity::find_by_id(node_id)
            .filter(bookmark_nodes::Column::LibraryId.eq(library_id))
            .one(txn)
            .await
            .map_err(|_| SyncError::DatabaseQuery { action })?
            .ok_or(SyncError::InvalidRequest {
                code: "node_not_in_preview_library",
                message: "mutation target does not belong to the selected library".to_owned(),
            })
    }

    async fn ensure_live_folder_in_library(
        &self,
        txn: &DatabaseTransaction,
        node_id: Uuid,
        library_id: Uuid,
        action: &'static str,
    ) -> SyncResult<bookmark_nodes::Model> {
        let node = self
            .ensure_node_in_library(txn, node_id, library_id, action)
            .await?;
        if node.node_type != "folder" || node.is_deleted {
            return Err(SyncError::InvalidRequest {
                code: "invalid_parent",
                message: "parent node must resolve to a live folder".to_owned(),
            });
        }
        Ok(node)
    }
}

fn map_bookmark_error(error: amagi_bookmarks::BookmarkError) -> SyncError {
    match error {
        amagi_bookmarks::BookmarkError::InvalidParent => SyncError::InvalidRequest {
            code: "invalid_parent",
            message: error.to_string(),
        },
        amagi_bookmarks::BookmarkError::InvalidNodeType => SyncError::InvalidRequest {
            code: "invalid_node_type",
            message: error.to_string(),
        },
        amagi_bookmarks::BookmarkError::InvalidUrl => SyncError::InvalidRequest {
            code: "invalid_url",
            message: error.to_string(),
        },
        amagi_bookmarks::BookmarkError::RootNodeImmutable => SyncError::InvalidRequest {
            code: "root_node_immutable",
            message: error.to_string(),
        },
        amagi_bookmarks::BookmarkError::LibraryNotFound => SyncError::LibraryNotFound,
        amagi_bookmarks::BookmarkError::DatabaseUnavailable => SyncError::DatabaseUnavailable,
        amagi_bookmarks::BookmarkError::DatabaseQuery { action } => {
            SyncError::DatabaseQuery { action }
        }
        amagi_bookmarks::BookmarkError::VaultNotSupportedInIter6 => {
            SyncError::VaultSyncNotSupported
        }
        amagi_bookmarks::BookmarkError::NodeNotFound => SyncError::InvalidRequest {
            code: "mapping_missing",
            message: "node target could not be resolved".to_owned(),
        },
        amagi_bookmarks::BookmarkError::Unauthenticated => SyncError::Unauthenticated,
        amagi_bookmarks::BookmarkError::Forbidden => SyncError::BrowserClientNotFound,
    }
}

fn conflict(conflict_type: &str, summary: &str, details: Value) -> SyncConflictView {
    SyncConflictView {
        conflict_type: conflict_type.to_owned(),
        summary: summary.to_owned(),
        details,
    }
}

fn validate_profile_create_request(request: &CreateSyncProfileRequest) -> SyncResult<()> {
    if request.name.trim().is_empty() {
        return Err(SyncError::InvalidRequest {
            code: "profile_name_required",
            message: "name is required".to_owned(),
        });
    }
    if request.mode.trim() != "manual" {
        return Err(SyncError::InvalidRequest {
            code: "invalid_profile_mode",
            message: "Iter11 only allows mode=manual".to_owned(),
        });
    }
    validate_default_direction(request.default_direction.trim())?;
    validate_conflict_policy(request.conflict_policy.trim())?;
    Ok(())
}

fn validate_profile_update_request(request: &UpdateSyncProfileRequest) -> SyncResult<()> {
    if let Some(name) = &request.name
        && name.trim().is_empty()
    {
        return Err(SyncError::InvalidRequest {
            code: "profile_name_required",
            message: "name is required".to_owned(),
        });
    }
    if let Some(default_direction) = &request.default_direction {
        validate_default_direction(default_direction.trim())?;
    }
    if let Some(conflict_policy) = &request.conflict_policy {
        validate_conflict_policy(conflict_policy.trim())?;
    }
    Ok(())
}

fn validate_default_direction(default_direction: &str) -> SyncResult<()> {
    if matches!(default_direction, "pull" | "push" | "bidirectional") {
        Ok(())
    } else {
        Err(SyncError::InvalidRequest {
            code: "invalid_default_direction",
            message: "defaultDirection must be pull, push, or bidirectional".to_owned(),
        })
    }
}

fn validate_conflict_policy(conflict_policy: &str) -> SyncResult<()> {
    if conflict_policy == "manual" {
        Ok(())
    } else {
        Err(SyncError::InvalidRequest {
            code: "invalid_conflict_policy",
            message: "Iter11 only allows conflictPolicy=manual".to_owned(),
        })
    }
}

fn normalize_target_request(
    request: &CreateSyncProfileTargetRequest,
) -> SyncResult<CreateSyncProfileTargetRequest> {
    let normalized_request = CreateSyncProfileTargetRequest {
        platform: trim_optional(&request.platform),
        device_type: trim_optional(&request.device_type),
        device_id: trim_optional(&request.device_id),
        browser_family: trim_optional(&request.browser_family),
        browser_client_id: trim_optional(&request.browser_client_id),
    };
    if normalized_request.platform.is_none()
        && normalized_request.device_type.is_none()
        && normalized_request.device_id.is_none()
        && normalized_request.browser_family.is_none()
        && normalized_request.browser_client_id.is_none()
    {
        return Err(SyncError::InvalidRequest {
            code: "target_selector_required",
            message: "at least one target selector field is required".to_owned(),
        });
    }
    Ok(normalized_request)
}

fn validate_profile_rule_create_request(
    request: &CreateSyncProfileRuleRequest,
) -> SyncResult<CreateSyncProfileRuleRequest> {
    validate_rule_action(request.action.trim())?;
    validate_rule_matcher(request.matcher_type.trim(), request.matcher_value.trim())?;
    Ok(CreateSyncProfileRuleRequest {
        rule_order: request.rule_order,
        action: request.action.trim().to_owned(),
        matcher_type: request.matcher_type.trim().to_owned(),
        matcher_value: request.matcher_value.trim().to_owned(),
        options: request.options.clone(),
    })
}

fn validate_profile_rule_update_request(
    request: &UpdateSyncProfileRuleRequest,
) -> SyncResult<UpdateSyncProfileRuleRequest> {
    let action = request.action.as_ref().map(|value| value.trim().to_owned());
    if let Some(action) = &action {
        validate_rule_action(action)?;
    }
    let matcher_type = request
        .matcher_type
        .as_ref()
        .map(|value| value.trim().to_owned());
    let matcher_value = request
        .matcher_value
        .as_ref()
        .map(|value| value.trim().to_owned());
    match (matcher_type.as_deref(), matcher_value.as_deref()) {
        (Some(matcher_type), Some(matcher_value)) => {
            validate_rule_matcher(matcher_type, matcher_value)?;
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(SyncError::InvalidRequest {
                code: "rule_matcher_update_incomplete",
                message: "matcherType and matcherValue must be updated together".to_owned(),
            });
        }
        (None, None) => {}
    }
    Ok(UpdateSyncProfileRuleRequest {
        rule_order: request.rule_order,
        action,
        matcher_type,
        matcher_value,
        options: request.options.clone(),
    })
}

fn validate_rule_action(action: &str) -> SyncResult<()> {
    if matches!(action, "include" | "exclude" | "readonly") {
        Ok(())
    } else {
        Err(SyncError::InvalidRequest {
            code: "invalid_rule_action",
            message: "action must be include, exclude, or readonly".to_owned(),
        })
    }
}

fn validate_rule_matcher(matcher_type: &str, matcher_value: &str) -> SyncResult<()> {
    match matcher_type {
        "library_kind" => {
            if matches!(matcher_value, "normal" | "vault") {
                Ok(())
            } else {
                Err(SyncError::InvalidRequest {
                    code: "invalid_library_kind",
                    message: "library_kind matcherValue must be normal or vault".to_owned(),
                })
            }
        }
        "folder_id" => {
            Uuid::parse_str(matcher_value).map_err(|_| SyncError::InvalidRequest {
                code: "folder_id_invalid",
                message: "folder_id matcherValue must be a UUID".to_owned(),
            })?;
            Ok(())
        }
        "folder_path" | "tag" => {
            if matcher_value.is_empty() {
                Err(SyncError::InvalidRequest {
                    code: "matcher_value_required",
                    message: "matcherValue must be non-empty".to_owned(),
                })
            } else {
                Ok(())
            }
        }
        _ => Err(SyncError::InvalidRequest {
            code: "invalid_matcher_type",
            message: "matcherType must be library_kind, folder_id, folder_path, or tag".to_owned(),
        }),
    }
}

fn trim_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_apply_result(summary_json: Value) -> SyncResult<SyncApplyResponse> {
    summary_json
        .get("applyResult")
        .cloned()
        .ok_or(SyncError::DatabaseQuery {
            action: "load stored sync apply result",
        })
        .and_then(|value| {
            serde_json::from_value(value).map_err(|_| SyncError::DatabaseQuery {
                action: "deserialize stored sync apply result",
            })
        })
}

async fn finish_read_txn<T>(txn: DatabaseTransaction, result: SyncResult<T>) -> SyncResult<T> {
    txn.rollback().await.map_err(|_| SyncError::DatabaseQuery {
        action: "rollback sync read transaction",
    })?;
    result
}

async fn finish_write_txn<T>(txn: DatabaseTransaction, result: SyncResult<T>) -> SyncResult<T> {
    match result {
        Ok(value) => {
            txn.commit().await.map_err(|_| SyncError::DatabaseQuery {
                action: "commit sync transaction",
            })?;
            Ok(value)
        }
        Err(error) => {
            txn.rollback().await.map_err(|_| SyncError::DatabaseQuery {
                action: "rollback sync transaction",
            })?;
            Err(error)
        }
    }
}

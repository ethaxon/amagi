use serde::{Deserialize, Serialize};
use serde_json::Value;

fn empty_json_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterClientRequest {
    pub device: DeviceRegistrationRequest,
    pub browser_client: BrowserClientRegistrationRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRegistrationRequest {
    pub device_id: Option<String>,
    pub device_name: String,
    pub device_type: String,
    pub platform: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserClientRegistrationRequest {
    pub browser_family: String,
    pub browser_profile_name: Option<String>,
    pub extension_instance_id: String,
    pub capabilities: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterClientResponse {
    pub device: DeviceView,
    pub browser_client: BrowserClientView,
    pub default_profile: SyncProfileView,
    pub cursor_summaries: Vec<CursorSummaryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionStartRequest {
    pub browser_client_id: String,
    pub preferred_profile_id: Option<String>,
    pub local_capability_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionStartResponse {
    pub browser_client: BrowserClientView,
    pub selected_profile: SyncProfileView,
    pub available_profiles: Vec<SyncProfileView>,
    pub libraries: Vec<SyncLibraryView>,
    pub cursors: Vec<CursorSummaryView>,
    pub server_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedRequest {
    pub browser_client_id: String,
    pub library_id: String,
    pub from_clock: i64,
    pub profile_id: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedResponse {
    pub browser_client_id: String,
    pub library_id: String,
    pub from_clock: i64,
    pub to_clock: i64,
    pub current_clock: i64,
    pub server_ops: Vec<ServerOpView>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewRequest {
    pub browser_client_id: String,
    pub profile_id: String,
    pub library_id: String,
    pub base_clock: i64,
    pub local_snapshot_summary: Value,
    pub local_mutations: Vec<LocalMutationInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalMutationInput {
    pub client_mutation_id: String,
    pub op: String,
    pub server_node_id: Option<String>,
    pub client_external_id: Option<String>,
    pub parent_server_node_id: Option<String>,
    pub parent_client_external_id: Option<String>,
    pub node_type: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub sort_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewResponse {
    pub preview_id: String,
    pub expires_at: String,
    pub summary: PreviewSummaryView,
    pub server_ops: Vec<ServerOpView>,
    pub accepted_local_mutations: Vec<AcceptedLocalMutationView>,
    pub conflicts: Vec<SyncConflictView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApplyRequest {
    pub preview_id: String,
    pub confirm: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApplyResponse {
    pub applied: bool,
    pub new_clock: i64,
    pub server_ops_to_apply_locally: Vec<ServerOpView>,
    pub created_mappings: Vec<NodeClientMappingView>,
    pub conflicts: Vec<SyncConflictView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorAckRequest {
    pub browser_client_id: String,
    pub library_id: String,
    pub applied_clock: i64,
    pub last_ack_rev_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSyncProfileRequest {
    pub name: String,
    pub mode: String,
    pub default_direction: String,
    pub conflict_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSyncProfileRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub default_direction: Option<String>,
    pub conflict_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSyncProfileTargetRequest {
    pub platform: Option<String>,
    pub device_type: Option<String>,
    pub device_id: Option<String>,
    pub browser_family: Option<String>,
    pub browser_client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSyncProfileRuleRequest {
    pub rule_order: i32,
    pub action: String,
    pub matcher_type: String,
    pub matcher_value: String,
    #[serde(default = "empty_json_object")]
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSyncProfileRuleRequest {
    pub rule_order: Option<i32>,
    pub action: Option<String>,
    pub matcher_type: Option<String>,
    pub matcher_value: Option<String>,
    pub options: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorAckResponse {
    pub cursor: CursorSummaryView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub device_name: String,
    pub device_type: String,
    pub platform: String,
    pub trust_level: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserClientView {
    pub id: String,
    pub device_id: String,
    pub browser_family: String,
    pub browser_profile_name: Option<String>,
    pub extension_instance_id: String,
    pub capabilities: Value,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProfileRuleView {
    pub id: String,
    pub rule_order: i32,
    pub action: String,
    pub matcher_type: String,
    pub matcher_value: String,
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProfileTargetView {
    pub id: String,
    pub platform: Option<String>,
    pub device_type: Option<String>,
    pub device_id: Option<String>,
    pub browser_family: Option<String>,
    pub browser_client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProfileView {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub default_direction: String,
    pub conflict_policy: String,
    pub enabled: bool,
    pub rules: Vec<SyncProfileRuleView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProfileDetailView {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub default_direction: String,
    pub conflict_policy: String,
    pub enabled: bool,
    pub rules: Vec<SyncProfileRuleView>,
    pub targets: Vec<SyncProfileTargetView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSummaryView {
    pub browser_client_id: String,
    pub library_id: String,
    pub last_applied_clock: i64,
    pub last_ack_rev_id: Option<String>,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncLibraryView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub projection: String,
    pub current_revision_clock: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerOpView {
    pub rev_id: String,
    pub node_id: String,
    pub op_type: String,
    pub logical_clock: i64,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedLocalMutationView {
    pub client_mutation_id: String,
    pub op: String,
    pub server_node_id: Option<String>,
    pub client_external_id: Option<String>,
    pub parent_server_node_id: Option<String>,
    pub node_type: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub sort_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSummaryView {
    pub server_to_local: usize,
    pub local_to_server_accepted: usize,
    pub conflicts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictView {
    pub conflict_type: String,
    pub summary: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeClientMappingView {
    pub browser_client_id: String,
    pub server_node_id: String,
    pub client_external_id: String,
}

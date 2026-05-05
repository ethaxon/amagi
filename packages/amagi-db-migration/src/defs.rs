use sea_orm_migration::prelude::*;

#[derive(DeriveIden, Clone, Copy)]
pub enum Users {
    Table,
    Id,
    Email,
    DisplayName,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum AuthUsers {
    Table,
    Id,
    UserId,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum OidcAccountBindings {
    Table,
    Id,
    AuthUserId,
    UserId,
    OidcSource,
    OidcSubject,
    OidcIdentityKey,
    ClaimsSnapshotJson,
    LastSeenAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Devices {
    Table,
    Id,
    UserId,
    DeviceName,
    DeviceType,
    Platform,
    TrustLevel,
    LastSeenAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum BrowserClients {
    Table,
    Id,
    DeviceId,
    BrowserFamily,
    BrowserProfileName,
    ExtensionInstanceId,
    CapabilitiesJson,
    LastSeenAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum Libraries {
    Table,
    Id,
    OwnerUserId,
    Kind,
    Name,
    VisibilityPolicyId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum BookmarkNodes {
    Table,
    Id,
    LibraryId,
    NodeType,
    ParentId,
    SortKey,
    Title,
    Url,
    UrlNormalized,
    ContentHash,
    IsDeleted,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum BookmarkMeta {
    Table,
    NodeId,
    Description,
    Tags,
    CanonicalUrl,
    PageTitle,
    FaviconAssetId,
    ReadingState,
    Starred,
    ExtraJson,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum LibraryHeads {
    Table,
    LibraryId,
    CurrentRevisionClock,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum NodeRevisions {
    Table,
    RevId,
    LibraryId,
    NodeId,
    ActorType,
    ActorId,
    OpType,
    PayloadJson,
    LogicalClock,
    CreatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncCursors {
    Table,
    BrowserClientId,
    LibraryId,
    LastAppliedClock,
    LastAckRevId,
    LastSyncAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum NodeClientMappings {
    Table,
    BrowserClientId,
    ServerNodeId,
    ClientExternalId,
    LastSeenHash,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncConflicts {
    Table,
    Id,
    BrowserClientId,
    LibraryId,
    ConflictType,
    State,
    Summary,
    DetailsJson,
    CreatedAt,
    ResolvedAt,
    ResolvedBy,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncProfiles {
    Table,
    Id,
    UserId,
    Name,
    Mode,
    DefaultDirection,
    ConflictPolicy,
    Enabled,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncProfileTargets {
    Table,
    Id,
    ProfileId,
    Platform,
    DeviceType,
    DeviceId,
    BrowserFamily,
    BrowserClientId,
    CreatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncProfileRules {
    Table,
    Id,
    ProfileId,
    RuleOrder,
    Action,
    MatcherType,
    MatcherValue,
    OptionsJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum SyncPreviews {
    Table,
    Id,
    UserId,
    BrowserClientId,
    LibraryId,
    BaseClock,
    ToClock,
    Status,
    RequestHash,
    SummaryJson,
    ServerOpsJson,
    AcceptedLocalMutationsJson,
    ConflictsJson,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
    AppliedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum VaultUnlockSessions {
    Table,
    Id,
    UserId,
    LibraryId,
    AuthContextJson,
    Acr,
    Amr,
    ExpiresAt,
    CreatedAt,
    RevokedAt,
}

#[derive(DeriveIden, Clone, Copy)]
pub enum AuditEvents {
    Table,
    Id,
    UserId,
    DeviceId,
    BrowserClientId,
    LibraryId,
    EventType,
    PayloadJson,
    CreatedAt,
}

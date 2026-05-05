export const LocalNodeType = {
	Folder: "folder",
	Bookmark: "bookmark",
	Separator: "separator",
} as const;

export type LocalNodeType = (typeof LocalNodeType)[keyof typeof LocalNodeType];

export const LocalMutationOp = {
	Create: "create",
	Update: "update",
	Move: "move",
	Delete: "delete",
	Restore: "restore",
} as const;

export type LocalMutationOp =
	(typeof LocalMutationOp)[keyof typeof LocalMutationOp];

export const ManualSyncStatus = {
	Synced: "synced",
	NeedsUserResolution: "needs-user-resolution",
	AwaitingConfirmation: "awaiting-confirmation",
	RecoveryRequired: "recovery-required",
} as const;

export type ManualSyncStatus =
	(typeof ManualSyncStatus)[keyof typeof ManualSyncStatus];

export interface DeviceRegistrationRequest {
	deviceId: string | null;
	deviceName: string;
	deviceType: string;
	platform: string;
}

export interface BrowserClientRegistrationRequest {
	browserFamily: string;
	browserProfileName: string | null;
	extensionInstanceId: string;
	capabilities: Record<string, unknown>;
}

export interface RegisterClientRequest {
	device: DeviceRegistrationRequest;
	browserClient: BrowserClientRegistrationRequest;
}

export interface SyncProfileRuleView {
	id: string;
	ruleOrder: number;
	action: string;
	matcherType: string;
	matcherValue: string;
	options: Record<string, unknown>;
}

export interface SyncProfileView {
	id: string;
	name: string;
	mode: string;
	defaultDirection: string;
	conflictPolicy: string;
	enabled: boolean;
	rules: SyncProfileRuleView[];
}

export interface DeviceView {
	id: string;
	deviceName: string;
	deviceType: string;
	platform: string;
	trustLevel: string;
	lastSeenAt: string | null;
}

export interface BrowserClientView {
	id: string;
	deviceId: string;
	browserFamily: string;
	browserProfileName: string | null;
	extensionInstanceId: string;
	capabilities: Record<string, unknown>;
	lastSeenAt: string | null;
}

export interface CursorSummaryView {
	browserClientId: string;
	libraryId: string;
	lastAppliedClock: number;
	lastAckRevId: string | null;
	lastSyncAt: string | null;
}

export interface SyncLibraryView {
	id: string;
	name: string;
	kind: string;
	projection: string;
	currentRevisionClock: number;
}

export interface RegisterClientResponse {
	device: DeviceView;
	browserClient: BrowserClientView;
	defaultProfile: SyncProfileView;
	cursorSummaries: CursorSummaryView[];
}

export interface SyncSessionStartRequest {
	browserClientId: string;
	preferredProfileId: string | null;
	localCapabilitySummary: Record<string, unknown>;
}

export interface SyncSessionStartResponse {
	browserClient: BrowserClientView;
	selectedProfile: SyncProfileView;
	availableProfiles: SyncProfileView[];
	libraries: SyncLibraryView[];
	cursors: CursorSummaryView[];
	serverTime: string;
}

export interface FeedRequest {
	browserClientId: string;
	libraryId: string;
	fromClock: number;
	profileId: string | null;
	limit: number | null;
}

export interface ServerOpView {
	revId: string;
	nodeId: string;
	opType: string;
	logicalClock: number;
	payload: Record<string, unknown>;
	createdAt: string;
}

export interface FeedResponse {
	browserClientId: string;
	libraryId: string;
	fromClock: number;
	toClock: number;
	currentClock: number;
	serverOps: ServerOpView[];
	nextCursor: number | null;
}

export interface LocalMutationInput {
	clientMutationId: string;
	op: LocalMutationOp;
	serverNodeId: string | null;
	clientExternalId: string | null;
	parentServerNodeId: string | null;
	parentClientExternalId: string | null;
	nodeType: LocalNodeType | null;
	title: string | null;
	url: string | null;
	sortKey: string | null;
}

export interface PreviewSummaryView {
	serverToLocal: number;
	localToServerAccepted: number;
	conflicts: number;
}

export interface SyncConflictView {
	conflictType: string;
	summary: string;
	details: Record<string, unknown>;
}

export interface AcceptedLocalMutationView {
	clientMutationId: string;
	op: LocalMutationOp;
	serverNodeId: string | null;
	clientExternalId: string | null;
	parentServerNodeId: string | null;
	nodeType: LocalNodeType | null;
	title: string | null;
	url: string | null;
	sortKey: string | null;
}

export interface SyncPreviewRequest {
	browserClientId: string;
	profileId: string;
	libraryId: string;
	baseClock: number;
	localSnapshotSummary: Record<string, unknown>;
	localMutations: LocalMutationInput[];
}

export interface SyncPreviewResponse {
	previewId: string;
	expiresAt: string;
	summary: PreviewSummaryView;
	serverOps: ServerOpView[];
	acceptedLocalMutations: AcceptedLocalMutationView[];
	conflicts: SyncConflictView[];
}

export interface NodeClientMappingView {
	browserClientId: string;
	serverNodeId: string;
	clientExternalId: string;
}

export interface SyncApplyRequest {
	previewId: string;
	confirm: boolean;
}

export interface SyncApplyResponse {
	applied: boolean;
	newClock: number;
	serverOpsToApplyLocally: ServerOpView[];
	createdMappings: NodeClientMappingView[];
	conflicts: SyncConflictView[];
}

export interface CursorAckRequest {
	browserClientId: string;
	libraryId: string;
	appliedClock: number;
	lastAckRevId: string | null;
}

export interface CursorAckResponse {
	cursor: CursorSummaryView;
}

export interface AmagiSyncApi {
	registerClient(
		request: RegisterClientRequest,
	): Promise<RegisterClientResponse>;
	startSession(
		request: SyncSessionStartRequest,
	): Promise<SyncSessionStartResponse>;
	feed(request: FeedRequest): Promise<FeedResponse>;
	preview(request: SyncPreviewRequest): Promise<SyncPreviewResponse>;
	apply(request: SyncApplyRequest): Promise<SyncApplyResponse>;
	ackCursor(request: CursorAckRequest): Promise<CursorAckResponse>;
}

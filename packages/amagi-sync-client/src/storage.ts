import type { LocalBookmarkTree } from "./local-tree";
import type {
	CursorSummaryView,
	SyncConflictView,
	SyncPreviewResponse,
} from "./types";

export interface PendingPreviewState {
	libraryId: string;
	preview: SyncPreviewResponse;
	storedAt: string;
	needsUserResolution: boolean;
}

export interface PendingRecoveryState {
	libraryId: string;
	previewId: string;
	newClock: number;
	storedAt: string;
	errorMessage: string;
	conflicts: SyncConflictView[];
}

export interface LastSyncStatus {
	state:
		| "idle"
		| "synced"
		| "needs-user-resolution"
		| "awaiting-confirmation"
		| "recovery-required";
	libraryId: string | null;
	updatedAt: string;
	message: string;
}

export interface SyncState {
	browserClientId: string | null;
	selectedProfileId: string | null;
	mappingsByClientExternalId: Record<string, string>;
	cursorsByLibraryId: Record<string, CursorSummaryView>;
	lastKnownTree: LocalBookmarkTree | null;
	pendingPreview: PendingPreviewState | null;
	pendingRecovery: PendingRecoveryState | null;
	lastSyncStatus: LastSyncStatus | null;
}

export interface SyncStorage {
	loadState(): Promise<SyncState>;
	saveState(state: SyncState): Promise<void>;
}

export function createEmptySyncState(): SyncState {
	return {
		browserClientId: null,
		selectedProfileId: null,
		mappingsByClientExternalId: {},
		cursorsByLibraryId: {},
		lastKnownTree: null,
		pendingPreview: null,
		pendingRecovery: null,
		lastSyncStatus: null,
	};
}

export function createMemorySyncStorage(
	initialState?: Partial<SyncState>,
): SyncStorage {
	let state: SyncState = {
		...createEmptySyncState(),
		...initialState,
		mappingsByClientExternalId: {
			...createEmptySyncState().mappingsByClientExternalId,
			...(initialState?.mappingsByClientExternalId ?? {}),
		},
		cursorsByLibraryId: {
			...createEmptySyncState().cursorsByLibraryId,
			...(initialState?.cursorsByLibraryId ?? {}),
		},
	};

	return {
		async loadState() {
			return structuredClone(state);
		},
		async saveState(nextState) {
			state = structuredClone(nextState);
		},
	};
}

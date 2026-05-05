import type { SyncAdapter } from "./adapter";
import { buildLocalApplyPlan } from "./apply-plan";
import { buildLocalMutations } from "./diff";
import { normalizeLocalTree } from "./normalize";
import { createEmptySyncState, type SyncStorage } from "./storage";
import type {
	AmagiSyncApi,
	CursorSummaryView,
	RegisterClientRequest,
	SyncPreviewResponse,
} from "./types";

export interface ManualSyncAuthContext {
	deviceName: string;
	deviceType: string;
	platform: string;
	browserFamily: string;
	browserProfileName?: string | null;
	extensionInstanceId: string;
}

export interface RunManualSyncOptions {
	api: AmagiSyncApi;
	adapter: SyncAdapter;
	storage: SyncStorage;
	auth: ManualSyncAuthContext;
	confirmApply(preview: SyncPreviewResponse): Promise<boolean> | boolean;
	now?: () => Date;
}

export interface ManualSyncResult {
	status:
		| "synced"
		| "needs-user-resolution"
		| "awaiting-confirmation"
		| "recovery-required";
	libraryId: string;
	preview: SyncPreviewResponse;
	newClock: number | null;
}

export async function runManualSync(
	options: RunManualSyncOptions,
): Promise<ManualSyncResult> {
	const now = options.now ?? (() => new Date());
	const existingState = await options.storage.loadState();
	let state = existingState ?? createEmptySyncState();
	const capabilities = await options.adapter.getCapabilities();
	const capabilitySummary: Record<string, unknown> = {
		canReadBookmarks: capabilities.canReadBookmarks,
		canWriteBookmarks: capabilities.canWriteBookmarks,
	};

	if (!state.browserClientId) {
		const registerRequest: RegisterClientRequest = {
			device: {
				deviceId: null,
				deviceName: options.auth.deviceName,
				deviceType: options.auth.deviceType,
				platform: options.auth.platform,
			},
			browserClient: {
				browserFamily: options.auth.browserFamily,
				browserProfileName: options.auth.browserProfileName ?? null,
				extensionInstanceId: options.auth.extensionInstanceId,
				capabilities: capabilitySummary,
			},
		};
		const registerResponse = await options.api.registerClient(registerRequest);
		state = {
			...state,
			browserClientId: registerResponse.browserClient.id,
			selectedProfileId: registerResponse.defaultProfile.id,
			cursorsByLibraryId: indexCursors(registerResponse.cursorSummaries),
		};
		await options.storage.saveState(state);
	}
	const browserClientId = state.browserClientId;
	if (!browserClientId) {
		throw new Error("manual sync registration must produce a browserClientId");
	}

	const session = await options.api.startSession({
		browserClientId,
		preferredProfileId: state.selectedProfileId,
		localCapabilitySummary: capabilitySummary,
	});
	const selectedLibrary = session.libraries.find(
		(library) => library.projection === "include",
	);
	if (!selectedLibrary) {
		throw new Error("manual sync requires at least one included library");
	}
	const cursor =
		state.cursorsByLibraryId[selectedLibrary.id] ??
		session.cursors.find((item) => item.libraryId === selectedLibrary.id) ??
		null;
	const baseClock = cursor?.lastAppliedClock ?? 0;
	const feed = await options.api.feed({
		browserClientId,
		libraryId: selectedLibrary.id,
		fromClock: baseClock,
		profileId: session.selectedProfile.id,
		limit: 100,
	});
	const currentTree = normalizeLocalTree(await options.adapter.loadTree());
	const diff = buildLocalMutations({
		currentTree,
		previousTree: state.lastKnownTree,
		mappingsByClientExternalId: state.mappingsByClientExternalId,
	});
	const preview = await options.api.preview({
		browserClientId,
		profileId: session.selectedProfile.id,
		libraryId: selectedLibrary.id,
		baseClock,
		localSnapshotSummary: { ...diff.localSnapshotSummary },
		localMutations: diff.localMutations,
	});

	if (preview.conflicts.length > 0) {
		state = {
			...state,
			selectedProfileId: session.selectedProfile.id,
			pendingPreview: {
				libraryId: selectedLibrary.id,
				preview,
				storedAt: now().toISOString(),
				needsUserResolution: true,
			},
			lastSyncStatus: {
				state: "needs-user-resolution",
				libraryId: selectedLibrary.id,
				updatedAt: now().toISOString(),
				message: `${preview.conflicts.length} conflict(s) require resolution`,
			},
		};
		await options.storage.saveState(state);
		return {
			status: "needs-user-resolution",
			libraryId: selectedLibrary.id,
			preview,
			newClock: null,
		};
	}

	const confirmed = await options.confirmApply(preview);
	if (!confirmed) {
		state = {
			...state,
			selectedProfileId: session.selectedProfile.id,
			pendingPreview: {
				libraryId: selectedLibrary.id,
				preview,
				storedAt: now().toISOString(),
				needsUserResolution: false,
			},
			lastSyncStatus: {
				state: "awaiting-confirmation",
				libraryId: selectedLibrary.id,
				updatedAt: now().toISOString(),
				message: "preview stored and awaiting explicit apply confirmation",
			},
		};
		await options.storage.saveState(state);
		return {
			status: "awaiting-confirmation",
			libraryId: selectedLibrary.id,
			preview,
			newClock: null,
		};
	}

	const applyResult = await options.api.apply({
		previewId: preview.previewId,
		confirm: true,
	});
	const mergedMappings = {
		...state.mappingsByClientExternalId,
		...Object.fromEntries(
			applyResult.createdMappings.map((mapping) => [
				mapping.clientExternalId,
				mapping.serverNodeId,
			]),
		),
	};
	const localApplyPlan = buildLocalApplyPlan({
		serverOps: applyResult.serverOpsToApplyLocally,
		mappingsByClientExternalId: mergedMappings,
		createdMappings: applyResult.createdMappings,
	});

	try {
		await options.adapter.applyLocalPlan(localApplyPlan);
	} catch (error) {
		state = {
			...state,
			selectedProfileId: session.selectedProfile.id,
			mappingsByClientExternalId: mergedMappings,
			pendingRecovery: {
				libraryId: selectedLibrary.id,
				previewId: preview.previewId,
				newClock: applyResult.newClock,
				storedAt: now().toISOString(),
				errorMessage: error instanceof Error ? error.message : String(error),
				conflicts: applyResult.conflicts,
			},
			lastSyncStatus: {
				state: "recovery-required",
				libraryId: selectedLibrary.id,
				updatedAt: now().toISOString(),
				message: "server apply succeeded but local adapter apply failed",
			},
		};
		await options.storage.saveState(state);
		return {
			status: "recovery-required",
			libraryId: selectedLibrary.id,
			preview,
			newClock: applyResult.newClock,
		};
	}

	const reloadedTree = normalizeLocalTree(await options.adapter.loadTree());
	const lastAckRevId =
		applyResult.serverOpsToApplyLocally.at(-1)?.revId ??
		feed.serverOps.at(-1)?.revId ??
		null;
	const ack = await options.api.ackCursor({
		browserClientId,
		libraryId: selectedLibrary.id,
		appliedClock: applyResult.newClock,
		lastAckRevId,
	});
	state = {
		...state,
		selectedProfileId: session.selectedProfile.id,
		mappingsByClientExternalId: mergedMappings,
		cursorsByLibraryId: {
			...state.cursorsByLibraryId,
			[selectedLibrary.id]: ack.cursor,
		},
		lastKnownTree: reloadedTree,
		pendingPreview: null,
		pendingRecovery: null,
		lastSyncStatus: {
			state: "synced",
			libraryId: selectedLibrary.id,
			updatedAt: now().toISOString(),
			message: "manual sync completed successfully",
		},
	};
	await options.storage.saveState(state);
	return {
		status: "synced",
		libraryId: selectedLibrary.id,
		preview,
		newClock: applyResult.newClock,
	};
}

function indexCursors(
	cursors: CursorSummaryView[],
): Record<string, CursorSummaryView> {
	return Object.fromEntries(
		cursors.map((cursor) => [cursor.libraryId, cursor]),
	);
}

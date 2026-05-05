import {
	createEmptySyncState,
	type SyncState,
	type SyncStorage,
} from "@ethaxon/amagi-sync-client";

import type { ChromiumStorageArea } from "./types";

const SYNC_STATE_KEY = "amagi.sync.state";

export function createChromiumStorage(
	storageArea: ChromiumStorageArea,
): SyncStorage {
	return {
		async loadState() {
			const stored = await storageArea.get(SYNC_STATE_KEY);
			const value = stored[SYNC_STATE_KEY];
			if (!isSyncStateLike(value)) {
				return createEmptySyncState();
			}
			return {
				...createEmptySyncState(),
				...value,
				mappingsByClientExternalId: {
					...createEmptySyncState().mappingsByClientExternalId,
					...(value.mappingsByClientExternalId ?? {}),
				},
				cursorsByLibraryId: {
					...createEmptySyncState().cursorsByLibraryId,
					...(value.cursorsByLibraryId ?? {}),
				},
			};
		},
		async saveState(state) {
			await storageArea.set({ [SYNC_STATE_KEY]: state });
		},
	};
}

function isSyncStateLike(value: unknown): value is Partial<SyncState> {
	return typeof value === "object" && value !== null;
}

import {
	createChromiumBookmarkAdapter,
	createChromiumStorage,
} from "@ethaxon/amagi-browser-adapter-chromium";
import {
	AmagiSyncApiClient,
	type ManualSyncResult,
	runManualSync,
} from "@ethaxon/amagi-sync-client";

import {
	defaultExtensionViewState,
	type ExtensionViewState,
	loadExtensionConfig,
} from "./state";

declare const chrome: {
	bookmarks: unknown;
	storage: { local: unknown };
	runtime: {
		onMessage: {
			addListener(
				handler: (
					message: { type?: string },
					sender: unknown,
					sendResponse: (
						response: ExtensionViewState | ManualSyncResult,
					) => void,
				) => boolean | void,
			): void;
		};
	};
};

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
	void handleMessage(message)
		.then((response) => sendResponse(response))
		.catch((error: unknown) => {
			sendResponse({
				status: "error",
				message: error instanceof Error ? error.message : String(error),
				previewSummary: null,
			});
		});
	return true;
});

async function handleMessage(message: {
	type?: string;
}): Promise<ExtensionViewState | ManualSyncResult> {
	const config = await loadExtensionConfig(chrome.storage.local as never);
	const syncStorage = createChromiumStorage(chrome.storage.local as never);
	if (message.type === "amagi.sync.status") {
		const syncState = await syncStorage.loadState();
		return syncState.lastSyncStatus
			? {
					status: syncState.lastSyncStatus.state,
					message: syncState.lastSyncStatus.message,
					previewSummary: syncState.pendingPreview?.preview.summary ?? null,
				}
			: defaultExtensionViewState;
	}
	if (!config.devBearerToken) {
		return {
			status: "error",
			message: "Set a dev bearer token in options before running manual sync.",
			previewSummary: null,
		};
	}

	const api = new AmagiSyncApiClient({
		baseUrl: config.apiBaseUrl,
		bearerToken: config.devBearerToken,
		oidcSource: config.oidcSource,
	});
	const adapter = createChromiumBookmarkAdapter({
		bookmarks: chrome.bookmarks as never,
		storage: { local: chrome.storage.local as never },
	});
	return runManualSync({
		api,
		adapter,
		storage: syncStorage,
		auth: {
			deviceName: "Chromium Browser",
			deviceType: "desktop",
			platform: navigator.userAgent,
			browserFamily: "chromium",
			browserProfileName: "Default",
			extensionInstanceId: "amagi-extension-dev",
		},
		confirmApply: () => message.type === "amagi.sync.apply",
	});
}

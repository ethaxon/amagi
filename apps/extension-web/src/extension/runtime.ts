import {
	AmagiAuthHost,
	createAmagiAuthClient,
	createBrowserStorageRecordStore,
} from "@ethaxon/amagi-auth-client";
import {
	AmagiSyncApiClient,
	type ManualSyncResult,
	runManualSync,
} from "@ethaxon/amagi-sync-client";
import {
	type BrowserLike,
	createWebExtBookmarkAdapter,
	createWebExtStorage,
	detectWebExtCapabilities,
} from "@ethaxon/amagi-webext";

import {
	type ExtensionMessage,
	type ExtensionMessageResponse,
	ExtensionMessageType,
	isExtensionMessage,
} from "./messaging";
import {
	defaultExtensionViewState,
	type ExtensionAuthState,
	ExtensionAuthStateStatus,
	type ExtensionConfig,
	type ExtensionViewState,
	ExtensionViewStateStatus,
	loadExtensionConfig,
	validateExtensionConfig,
} from "./state";

interface ExtensionAuthBrowserLike extends BrowserLike {
	runtime?: BrowserLike["runtime"] & {
		getURL?: (
			path: `/options.html${string}` | `/popup.html${string}`,
		) => string;
	};
	tabs?: {
		create?: (createProperties: { url?: string }) => Promise<unknown>;
	};
}

export function createBackgroundRuntime(options: { browser: BrowserLike }) {
	return {
		async handleMessage(message: unknown): Promise<ExtensionMessageResponse> {
			if (!isExtensionMessage(message)) {
				return {
					status: ExtensionViewStateStatus.Error,
					message: "unsupported extension message",
					previewSummary: null,
				};
			}

			const syncStorage = createWebExtStorage({
				storageArea: options.browser.storage?.local ?? missingStorageArea(),
			});
			if (message.type === ExtensionMessageType.Status) {
				return readStatus(syncStorage);
			}

			const config = await loadExtensionConfig(
				options.browser.storage?.local ?? missingStorageArea(),
			);
			const validatedConfig = validateExtensionConfig(config);
			if (!validatedConfig.isValid) {
				return {
					status: ExtensionViewStateStatus.Error,
					message: validatedConfig.message,
					previewSummary: null,
				};
			}
			const normalizedConfig = validatedConfig.config;
			const authorizationHeader = await resolveManualSyncAuthorizationHeader(
				options.browser,
				normalizedConfig,
			);
			if (!authorizationHeader) {
				return {
					status: ExtensionViewStateStatus.Error,
					message:
						"Log in from the popup or options page before running manual sync, or set the advanced dev bearer fallback.",
					previewSummary: null,
				};
			}

			const capabilities = await detectWebExtCapabilities(options.browser);
			if (!capabilities.canReadBookmarks || !capabilities.canWriteBookmarks) {
				return {
					status: ExtensionViewStateStatus.Error,
					message: "browser bookmarks API is unavailable for manual sync.",
					previewSummary: null,
				};
			}
			if (!capabilities.canUseStorage) {
				return {
					status: ExtensionViewStateStatus.Error,
					message: "browser storage API is unavailable for manual sync.",
					previewSummary: null,
				};
			}

			return runManualSync({
				api: new AmagiSyncApiClient({
					baseUrl: normalizedConfig.apiBaseUrl,
					bearerToken: stripBearerPrefix(authorizationHeader),
					oidcSource: normalizedConfig.oidcSource,
				}),
				adapter: createWebExtBookmarkAdapter({ browser: options.browser }),
				storage: syncStorage,
				auth: toManualSyncAuthContext(capabilities),
				confirmApply: () => message.type === ExtensionMessageType.Apply,
			});
		},
	};
}

export async function requestExtensionStatus(
	browser: BrowserLike,
): Promise<ExtensionViewState> {
	const response = await browser.runtime?.sendMessage?.({
		type: ExtensionMessageType.Status,
	});
	return isExtensionViewState(response)
		? response
		: {
				status: ExtensionViewStateStatus.Error,
				message:
					"extension background runtime did not return a status response.",
				previewSummary: null,
			};
}

export async function requestManualSync(
	browser: BrowserLike,
	message: ExtensionMessage,
): Promise<ManualSyncResult | ExtensionViewState> {
	const response = await browser.runtime?.sendMessage?.(message);
	return isManualSyncResult(response) || isExtensionViewState(response)
		? response
		: {
				status: ExtensionViewStateStatus.Error,
				message:
					"extension background runtime did not return a manual sync response.",
				previewSummary: null,
			};
}

export async function loadExtensionAuthState(
	browser: ExtensionAuthBrowserLike,
): Promise<ExtensionAuthState> {
	const config = await loadBrowserExtensionConfig(browser);
	const validatedConfig = validateExtensionConfig(config);
	if (!validatedConfig.isValid) {
		return {
			status: ExtensionAuthStateStatus.Error,
			message: validatedConfig.message,
			displayName: null,
			subject: null,
			usesDevBearerFallback: false,
		};
	}

	try {
		const authClient = createExtensionAuthClient(
			browser,
			validatedConfig.config,
		);
		const snapshot = await authClient.ensureReady();
		const authorizationHeader = authClient.authorizationHeader();
		const usesDevBearerFallback =
			validatedConfig.config.devBearerToken.length > 0;

		if (!snapshot || !authorizationHeader) {
			return {
				status: ExtensionAuthStateStatus.Unauthenticated,
				message: usesDevBearerFallback
					? "No token-set session is active. Advanced dev bearer fallback is configured."
					: "No token-set session is active. Log in from this extension to enable manual sync.",
				displayName: null,
				subject: null,
				usesDevBearerFallback,
			};
		}

		await authClient.loadBoundUserInfo();

		return {
			status: ExtensionAuthStateStatus.Authenticated,
			message:
				"Authenticated via backend-oidc token-set. Manual sync will use the shared browser-owned bearer principal.",
			displayName: readPrincipalDisplayName(snapshot),
			subject: readPrincipalSubject(snapshot),
			usesDevBearerFallback,
		};
	} catch (error: unknown) {
		return {
			status: ExtensionAuthStateStatus.Error,
			message: error instanceof Error ? error.message : String(error),
			displayName: null,
			subject: null,
			usesDevBearerFallback: validatedConfig.config.devBearerToken.length > 0,
		};
	}
}

export async function startExtensionLogin(
	browser: ExtensionAuthBrowserLike,
): Promise<void> {
	const config = await loadBrowserExtensionConfig(browser);
	const validatedConfig = validateExtensionConfig(config);
	if (!validatedConfig.isValid) {
		throw new Error(validatedConfig.message);
	}

	const authClient = createExtensionAuthClient(browser, validatedConfig.config);
	const loginUrl = authClient.createLoginUrl();
	if (browser.tabs?.create) {
		await browser.tabs.create({ url: loginUrl });
		return;
	}
	if (typeof globalThis.location?.assign === "function") {
		globalThis.location.assign(loginUrl);
		return;
	}
	throw new Error("browser tabs API is unavailable for launching auth");
}

export async function clearExtensionAuth(
	browser: ExtensionAuthBrowserLike,
): Promise<void> {
	const config = await loadBrowserExtensionConfig(browser);
	const validatedConfig = validateExtensionConfig(config);
	if (!validatedConfig.isValid) {
		throw new Error(validatedConfig.message);
	}

	const authClient = createExtensionAuthClient(browser, validatedConfig.config);
	await authClient.logout();
}

export function manualSyncResultToViewState(
	result: ManualSyncResult | ExtensionViewState,
): ExtensionViewState {
	if ("previewSummary" in result) {
		return result;
	}
	return {
		status: result.status,
		message: describeManualSyncResult(result),
		previewSummary: result.preview.summary,
	};
}

export async function loadBrowserExtensionConfig(
	browser: BrowserLike,
): Promise<ExtensionConfig> {
	return loadExtensionConfig(browser.storage?.local ?? missingStorageArea());
}

export async function saveBrowserExtensionConfig(
	browser: BrowserLike,
	config: ExtensionConfig,
): Promise<void> {
	const storageArea = browser.storage?.local;
	if (!storageArea) {
		throw new Error("browser storage API is unavailable");
	}
	const validatedConfig = validateExtensionConfig(config);
	if (!validatedConfig.isValid) {
		throw new Error(validatedConfig.message);
	}
	const { saveExtensionConfig } = await import("./state");
	await saveExtensionConfig(storageArea, validatedConfig.config);
}

async function readStatus(syncStorage: ReturnType<typeof createWebExtStorage>) {
	const syncState = await syncStorage.loadState();
	return syncState.lastSyncStatus
		? {
				status: syncState.lastSyncStatus.state,
				message: syncState.lastSyncStatus.message,
				previewSummary: syncState.pendingPreview?.preview.summary ?? null,
			}
		: defaultExtensionViewState;
}

function createExtensionAuthClient(
	browser: ExtensionAuthBrowserLike,
	config: ExtensionConfig,
) {
	const storageArea = getExtensionAuthStorageArea(browser);
	const optionsPageUrl = browser.runtime?.getURL?.("/options.html");

	return createAmagiAuthClient({
		baseUrl: config.apiBaseUrl,
		oidcSource: config.oidcSource,
		host: AmagiAuthHost.Extension,
		defaultPostAuthRedirectUri: optionsPageUrl,
		persistentStore: createBrowserStorageRecordStore({
			storageArea,
			prefix: "amagi.extension.auth.persistent.",
		}),
		sessionStore: createBrowserStorageRecordStore({
			storageArea,
			prefix: "amagi.extension.auth.session.",
		}),
	});
}

async function resolveManualSyncAuthorizationHeader(
	browser: ExtensionAuthBrowserLike,
	config: ExtensionConfig,
) {
	const authClient = createExtensionAuthClient(browser, config);
	const snapshot = await authClient.ensureReady();
	const authorizationHeader = authClient.authorizationHeader();
	if (snapshot && authorizationHeader) {
		await authClient.loadBoundUserInfo();
		return authorizationHeader;
	}

	const fallbackToken = config.devBearerToken.trim();
	return fallbackToken ? `Bearer ${fallbackToken}` : null;
}

function toManualSyncAuthContext(capabilities: {
	browserFamily?: string;
	manifestVersion?: number;
}) {
	return {
		deviceName: "Amagi WebExtension",
		deviceType: "desktop",
		platform: globalThis.navigator?.userAgent ?? "webext",
		browserFamily: capabilities.browserFamily ?? "unknown",
		browserProfileName: capabilities.manifestVersion === 2 ? "MV2" : "Default",
		extensionInstanceId: `amagi-extension-${capabilities.browserFamily ?? "unknown"}`,
	};
}

function describeManualSyncResult(result: ManualSyncResult): string {
	switch (result.status) {
		case "synced":
			return "manual sync completed successfully";
		case "needs-user-resolution":
			return "manual sync preview requires user resolution";
		case "awaiting-confirmation":
			return "manual sync preview is ready for confirmation";
		case "recovery-required":
			return "manual sync requires recovery before retry";
	}
}

function getExtensionAuthStorageArea(browser: ExtensionAuthBrowserLike) {
	const storageArea = browser.storage?.local;
	if (!storageArea) {
		throw new Error("browser storage API is unavailable");
	}

	return {
		get: storageArea.get.bind(storageArea),
		set: storageArea.set.bind(storageArea),
		remove:
			typeof (storageArea as { remove?: unknown }).remove === "function"
				? (
						storageArea as unknown as {
							remove: (keys: string | string[]) => Promise<void>;
						}
					).remove.bind(storageArea)
				: async () => {},
	};
}

function stripBearerPrefix(authorizationHeader: string) {
	return authorizationHeader.replace(/^Bearer\s+/u, "");
}

function readPrincipalDisplayName(
	snapshot: {
		metadata?: { principal?: { displayName?: unknown } };
	} | null,
) {
	return typeof snapshot?.metadata?.principal?.displayName === "string"
		? snapshot.metadata.principal.displayName
		: null;
}

function readPrincipalSubject(
	snapshot: {
		metadata?: { principal?: { subject?: unknown } };
	} | null,
) {
	return typeof snapshot?.metadata?.principal?.subject === "string"
		? snapshot.metadata.principal.subject
		: null;
}

function isExtensionViewState(value: unknown): value is ExtensionViewState {
	return (
		typeof value === "object" &&
		value !== null &&
		"status" in value &&
		typeof value.status === "string" &&
		"message" in value &&
		typeof value.message === "string" &&
		"previewSummary" in value
	);
}

function isManualSyncResult(value: unknown): value is ManualSyncResult {
	return (
		typeof value === "object" &&
		value !== null &&
		"status" in value &&
		typeof value.status === "string" &&
		"preview" in value &&
		typeof value.preview === "object" &&
		value.preview !== null
	);
}

function missingStorageArea() {
	return {
		async get() {
			return {};
		},
		async set() {
			throw new Error("browser storage API is unavailable");
		},
	};
}

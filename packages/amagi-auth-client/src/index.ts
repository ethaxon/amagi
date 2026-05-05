import {
	createInMemoryRecordStore,
	type RecordStore,
} from "@securitydept/client";
import {
	type BootstrapBackendOidcModeClientOptions,
	bootstrapBackendOidcModeClient,
	buildAuthorizeUrlReturningToCurrent,
	type CreateBackendOidcModeBrowserClientOptions,
	createBackendOidcModeBrowserClient,
	createBackendOidcModeCallbackFragmentStore,
} from "@securitydept/token-set-context-client/backend-oidc-mode/web";
import type { AuthSnapshot } from "@securitydept/token-set-context-client/orchestration";

export const AmagiAuthHost = {
	Dashboard: "dashboard",
	Extension: "extension",
	Test: "test",
} as const;

export type AmagiAuthHost = (typeof AmagiAuthHost)[keyof typeof AmagiAuthHost];

export const AMAGI_DEFAULT_OIDC_SOURCE = "default";

export interface AmagiAuthRoutePaths {
	loginPath: string;
	callbackPath: string;
	refreshPath: string;
	metadataRedeemPath: string;
	userInfoPath: string;
}

export interface BrowserStorageAreaLike {
	get(
		keys?: string | string[] | Record<string, unknown> | null,
	): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
	remove(keys: string | string[]): Promise<void>;
}

export interface CreateBrowserStorageRecordStoreOptions {
	storageArea: BrowserStorageAreaLike;
	prefix?: string;
}

export interface CreateMemoryRecordStoreOptions {
	prefix?: string;
}

export interface CreateAmagiAuthClientOptions {
	baseUrl: string;
	oidcSource?: string;
	host?: AmagiAuthHost;
	defaultPostAuthRedirectUri?: string;
	persistentStore?: RecordStore;
	sessionStore?: RecordStore;
	persistentStateKey?: string;
	transport?: CreateBackendOidcModeBrowserClientOptions["transport"];
	fetchTransport?: CreateBackendOidcModeBrowserClientOptions["fetchTransport"];
	scheduler?: CreateBackendOidcModeBrowserClientOptions["scheduler"];
	clock?: CreateBackendOidcModeBrowserClientOptions["clock"];
	logger?: CreateBackendOidcModeBrowserClientOptions["logger"];
	traceSink?: CreateBackendOidcModeBrowserClientOptions["traceSink"];
	resumeReconciliation?: CreateBackendOidcModeBrowserClientOptions["resumeReconciliation"];
	resumeReconciliationOptions?: CreateBackendOidcModeBrowserClientOptions["resumeReconciliationOptions"];
	fetchImpl?: typeof fetch;
}

export interface EnsureAmagiAuthReadyOptions {
	location?: BootstrapBackendOidcModeClientOptions["location"];
	history?: BootstrapBackendOidcModeClientOptions["history"];
}

export interface LoginToAmagiOptions {
	postAuthRedirectUri?: string;
	location?: Pick<Location, "href">;
	navigate?: (url: string) => void;
}

export interface LoadAmagiBoundUserInfoOptions {
	fetchImpl?: typeof fetch;
}

export interface AmagiBoundUserInfoResponse {
	source: string;
	userInfo: Record<string, unknown>;
	principalResolution: unknown;
}

export interface AmagiAuthClient {
	readonly baseUrl: string;
	readonly oidcSource: string;
	readonly paths: AmagiAuthRoutePaths;
	ensureReady(
		options?: EnsureAmagiAuthReadyOptions,
	): Promise<AuthSnapshot | null>;
	authSnapshot(): AuthSnapshot | null;
	authorizationHeader(): string | null;
	isAuthenticated(): boolean;
	createLoginUrl(options?: LoginToAmagiOptions): string;
	login(options?: LoginToAmagiOptions): string;
	logout(): Promise<void>;
	loadBoundUserInfo(
		options?: LoadAmagiBoundUserInfoOptions,
	): Promise<AmagiBoundUserInfoResponse>;
}

export function createAmagiAuthRoutePaths(options: {
	oidcSource?: string;
}): AmagiAuthRoutePaths {
	const oidcSource = normalizeOidcSource(options.oidcSource);
	return {
		loginPath: `/api/auth/token-set/oidc/source/${oidcSource}/start`,
		callbackPath: `/auth/token-set/oidc/source/${oidcSource}/callback`,
		refreshPath: `/api/auth/token-set/oidc/source/${oidcSource}/refresh`,
		metadataRedeemPath: `/api/auth/token-set/oidc/source/${oidcSource}/metadata/redeem`,
		userInfoPath: `/api/auth/token-set/oidc/source/${oidcSource}/user-info`,
	};
}

export function createBrowserStorageRecordStore(
	options: CreateBrowserStorageRecordStoreOptions,
): RecordStore {
	const prefix = options.prefix ?? "";

	return {
		async get(key) {
			const stored = await options.storageArea.get(prefix + key);
			const value = stored[prefix + key];
			return typeof value === "string" ? value : null;
		},
		async set(key, value) {
			await options.storageArea.set({ [prefix + key]: value });
		},
		async take(key) {
			const storageKey = prefix + key;
			const stored = await options.storageArea.get(storageKey);
			const value = stored[storageKey];
			await options.storageArea.remove(storageKey);
			return typeof value === "string" ? value : null;
		},
		async remove(key) {
			await options.storageArea.remove(prefix + key);
		},
	};
}

export function createMemoryRecordStore(
	options: CreateMemoryRecordStoreOptions = {},
): RecordStore {
	if (!options.prefix) {
		return createInMemoryRecordStore();
	}

	const delegate = createInMemoryRecordStore();
	return {
		get(key) {
			return delegate.get(options.prefix + key);
		},
		set(key, value) {
			return delegate.set(options.prefix + key, value);
		},
		take(key) {
			return delegate.take?.(options.prefix + key) ?? Promise.resolve(null);
		},
		remove(key) {
			return delegate.remove(options.prefix + key);
		},
	};
}

export function createAmagiAuthClient(
	options: CreateAmagiAuthClientOptions,
): AmagiAuthClient {
	const baseUrl = normalizeBaseUrl(options.baseUrl);
	const oidcSource = normalizeOidcSource(options.oidcSource);
	const host = options.host ?? AmagiAuthHost.Dashboard;
	const paths = createAmagiAuthRoutePaths({ oidcSource });
	const persistentStateKey =
		options.persistentStateKey ??
		buildPersistentStateKey({ baseUrl, host, oidcSource });
	const callbackFragmentKey = `${persistentStateKey}:callback-fragment`;
	const sessionStore = options.sessionStore;
	const callbackFragmentStore = createBackendOidcModeCallbackFragmentStore({
		sessionStore,
		key: callbackFragmentKey,
	});
	const client = createBackendOidcModeBrowserClient({
		baseUrl,
		defaultPostAuthRedirectUri: options.defaultPostAuthRedirectUri,
		persistentStateKey,
		loginPath: paths.loginPath,
		refreshPath: paths.refreshPath,
		metadataRedeemPath: paths.metadataRedeemPath,
		userInfoPath: paths.userInfoPath,
		persistentStore: options.persistentStore,
		sessionStore,
		transport: options.transport,
		fetchTransport: options.fetchTransport,
		scheduler: options.scheduler,
		clock: options.clock,
		logger: options.logger,
		traceSink: options.traceSink,
		resumeReconciliation: options.resumeReconciliation,
		resumeReconciliationOptions: options.resumeReconciliationOptions,
	});
	const fetchImpl = options.fetchImpl ?? getBrowserFetch();
	let currentSnapshot: AuthSnapshot | null = client.state.get();

	return {
		baseUrl,
		oidcSource,
		paths,
		async ensureReady(readyOptions = {}) {
			const location = readyOptions.location ?? getBrowserLocation();
			const history = readyOptions.history ?? getBrowserHistory();

			if (location && history) {
				const result = await bootstrapBackendOidcModeClient(client, {
					location,
					history,
					callbackFragmentStore,
				});
				if (result.snapshot) {
					client.restoreState(result.snapshot);
				}
				currentSnapshot = result.snapshot;
				return result.snapshot;
			}

			currentSnapshot = await client.restorePersistedState();
			return currentSnapshot;
		},
		authSnapshot() {
			return currentSnapshot;
		},
		authorizationHeader() {
			return currentSnapshot
				? `Bearer ${currentSnapshot.tokens.accessToken}`
				: null;
		},
		isAuthenticated() {
			return currentSnapshot !== null;
		},
		createLoginUrl(loginOptions = {}) {
			if (loginOptions.postAuthRedirectUri) {
				return client.authorizeUrl(loginOptions.postAuthRedirectUri);
			}

			if (options.defaultPostAuthRedirectUri) {
				return client.authorizeUrl(options.defaultPostAuthRedirectUri);
			}

			const location = loginOptions.location ?? getBrowserLocation();
			if (!location) {
				throw new Error(
					"postAuthRedirectUri is required when no browser location is available.",
				);
			}

			return buildAuthorizeUrlReturningToCurrent(client, location);
		},
		login(loginOptions = {}) {
			const loginUrl = this.createLoginUrl(loginOptions);
			if (loginOptions.navigate) {
				loginOptions.navigate(loginUrl);
			} else if (typeof globalThis.location?.assign === "function") {
				globalThis.location.assign(loginUrl);
			}
			return loginUrl;
		},
		async logout() {
			await callbackFragmentStore.clear();
			await client.clearState();
			currentSnapshot = null;
		},
		async loadBoundUserInfo(loadOptions = {}) {
			const snapshot = currentSnapshot;
			const header = this.authorizationHeader();

			if (!header || !snapshot?.tokens.idToken) {
				throw new Error(
					"authenticated token-set state with idToken is required before loading bound user info.",
				);
			}

			const response = await (loadOptions.fetchImpl ?? fetchImpl)(
				new URL(paths.userInfoPath, baseUrl),
				{
					method: "POST",
					headers: {
						accept: "application/json",
						authorization: header,
						"content-type": "application/json",
					},
					body: JSON.stringify({ id_token: snapshot.tokens.idToken }),
				},
			);

			const text = await response.text();
			const parsed = text ? tryParseJson(text) : null;
			if (!response.ok) {
				throw new Error(text || `${response.status} ${response.statusText}`);
			}

			if (!isAmagiBoundUserInfoResponsePayload(parsed)) {
				throw new Error("amagi user-info endpoint returned an invalid payload");
			}

			return {
				source: parsed.source,
				userInfo: parsed.user_info,
				principalResolution: parsed.principal_resolution,
			};
		},
	};
}

function buildPersistentStateKey(options: {
	baseUrl: string;
	host: AmagiAuthHost;
	oidcSource: string;
}): string {
	return `amagi.auth:${options.host}:${options.oidcSource}:${options.baseUrl}`;
}

function normalizeBaseUrl(value: string): string {
	const trimmed = value.trim();
	return trimmed.endsWith("/") ? trimmed : `${trimmed}/`;
}

function normalizeOidcSource(value: string | undefined): string {
	const trimmed = value?.trim();
	return trimmed ? trimmed : AMAGI_DEFAULT_OIDC_SOURCE;
}

function getBrowserLocation(): Pick<Location, "href" | "hash"> | undefined {
	if (typeof globalThis.location?.href !== "string") {
		return undefined;
	}

	return {
		href: globalThis.location.href,
		hash: globalThis.location.hash,
	};
}

function getBrowserHistory(): Pick<History, "replaceState"> | undefined {
	if (typeof globalThis.history?.replaceState !== "function") {
		return undefined;
	}

	return {
		replaceState: globalThis.history.replaceState.bind(globalThis.history),
	};
}

function getBrowserFetch(): typeof fetch {
	if (typeof globalThis.fetch !== "function") {
		throw new Error("fetch is not available in the current environment.");
	}

	return globalThis.fetch.bind(globalThis);
}

function tryParseJson(value: string): unknown | null {
	try {
		return JSON.parse(value) as unknown;
	} catch {
		return null;
	}
}

function isAmagiBoundUserInfoResponsePayload(value: unknown): value is {
	source: string;
	user_info: Record<string, unknown>;
	principal_resolution: unknown;
} {
	return (
		typeof value === "object" &&
		value !== null &&
		"source" in value &&
		typeof value.source === "string" &&
		"user_info" in value &&
		typeof value.user_info === "object" &&
		value.user_info !== null &&
		"principal_resolution" in value
	);
}

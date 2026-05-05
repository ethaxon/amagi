import { describe, expect, it, vi } from "vitest";

import {
	AMAGI_DEFAULT_OIDC_SOURCE,
	AmagiAuthHost,
	createAmagiAuthClient,
	createAmagiAuthRoutePaths,
	createBrowserStorageRecordStore,
	createMemoryRecordStore,
} from "../index";

describe("amagi auth client", () => {
	it("uses default source-aware route paths", () => {
		expect(createAmagiAuthRoutePaths({})).toEqual({
			loginPath: "/api/auth/token-set/oidc/source/default/start",
			callbackPath: "/auth/token-set/oidc/source/default/callback",
			refreshPath: "/api/auth/token-set/oidc/source/default/refresh",
			metadataRedeemPath:
				"/api/auth/token-set/oidc/source/default/metadata/redeem",
			userInfoPath: "/api/auth/token-set/oidc/source/default/user-info",
		});
		expect(AMAGI_DEFAULT_OIDC_SOURCE).toBe("default");
	});

	it("bootstraps empty state when no callback fragment or persisted state exists", async () => {
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		await expect(
			client.ensureReady({
				location: { href: "https://app.example.com/dashboard", hash: "" },
				history: { replaceState() {} },
			}),
		).resolves.toBeNull();
		expect(client.authorizationHeader()).toBeNull();
	});

	it("hydrates callback fragment state and produces an authorization header", async () => {
		const history = {
			replacedUrl: "",
			replaceState(_data: unknown, _unused: string, url?: string) {
				this.replacedUrl = url ?? "";
			},
		};
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		const snapshot = await client.ensureReady({
			location: {
				href: "https://app.example.com/auth/token-set/oidc/source/default/callback#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
				hash: "#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
			},
			history,
		});

		expect(snapshot?.tokens.accessToken).toBe("callback-at");
		expect(snapshot?.tokens.idToken).toBe("callback-idt");
		expect(client.authorizationHeader()).toBe("Bearer callback-at");
		expect(history.replacedUrl).toBe(
			"/auth/token-set/oidc/source/default/callback",
		);
	});

	it("restores persisted state when history is unavailable", async () => {
		const persistentStore = createMemoryRecordStore({ prefix: "persistent:" });
		const sessionStore = createMemoryRecordStore({ prefix: "session:" });
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore,
			sessionStore,
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		await client.ensureReady({
			location: {
				href: "https://app.example.com/auth/token-set/oidc/source/default/callback#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2027-01-01T00%3A05%3A00Z",
				hash: "#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2027-01-01T00%3A05%3A00Z",
			},
			history: { replaceState() {} },
		});

		const backgroundClient = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore,
			sessionStore,
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		const restored = await backgroundClient.ensureReady({
			location: {
				href: "chrome-extension://example/background.html",
				hash: "",
			},
		});

		expect(restored?.tokens.accessToken).toBe("callback-at");
		expect(backgroundClient.authorizationHeader()).toBe("Bearer callback-at");
	});

	it("clears persisted auth state on logout", async () => {
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		await client.ensureReady({
			location: {
				href: "https://app.example.com/auth/token-set/oidc/source/default/callback#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
				hash: "#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
			},
			history: { replaceState() {} },
		});

		await client.logout();

		expect(client.authorizationHeader()).toBeNull();
		expect(client.authSnapshot()).toBeNull();
	});

	it("prefers defaultPostAuthRedirectUri over the current page when building login URLs", () => {
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			defaultPostAuthRedirectUri: "chrome-extension://example/options.html",
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		const loginUrl = client.createLoginUrl({
			location: { href: "chrome-extension://example/popup.html" },
		});
		const redirect = new URL(loginUrl).searchParams.get(
			"post_auth_redirect_uri",
		);

		expect(redirect).toBe("chrome-extension://example/options.html");
	});

	it("lets an explicit postAuthRedirectUri override the default redirect", () => {
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			defaultPostAuthRedirectUri: "chrome-extension://example/options.html",
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
		});

		const loginUrl = client.createLoginUrl({
			postAuthRedirectUri: "chrome-extension://example/override.html",
			location: { href: "chrome-extension://example/popup.html" },
		});
		const redirect = new URL(loginUrl).searchParams.get(
			"post_auth_redirect_uri",
		);

		expect(redirect).toBe("chrome-extension://example/override.html");
	});

	it("loads bound user info with idToken and bearer authorization", async () => {
		const fetchImpl = vi.fn(
			async (_input: URL | RequestInfo, init?: RequestInit) => {
				expect(init?.headers).toMatchObject({
					authorization: "Bearer callback-at",
					"content-type": "application/json",
				});
				expect(init?.body).toBe(JSON.stringify({ id_token: "callback-idt" }));
				return new Response(
					JSON.stringify({
						source: "default",
						user_info: { subject: "user-1" },
						principal_resolution: { status: "resolved" },
					}),
					{ status: 200 },
				);
			},
		);
		const client = createAmagiAuthClient({
			baseUrl: "https://auth.example.com",
			host: AmagiAuthHost.Test,
			persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
			sessionStore: createMemoryRecordStore({ prefix: "session:" }),
			transport: {
				async execute() {
					return { status: 500, headers: {}, body: null };
				},
			},
			scheduler: {
				setTimeout() {
					return { cancel() {} };
				},
			},
			clock: { now: () => Date.now() },
			fetchImpl: fetchImpl as typeof fetch,
		});

		await client.ensureReady({
			location: {
				href: "https://app.example.com/auth/token-set/oidc/source/default/callback#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
				hash: "#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2026-01-01T00%3A05%3A00Z",
			},
			history: { replaceState() {} },
		});

		await expect(client.loadBoundUserInfo()).resolves.toEqual({
			source: "default",
			userInfo: { subject: "user-1" },
			principalResolution: { status: "resolved" },
		});
	});

	it("binds the default global fetch to the current global object", async () => {
		const originalFetch = globalThis.fetch;
		const globalFetch = vi.fn(function (
			this: typeof globalThis,
			_input: URL | RequestInfo,
			_init?: RequestInit,
		) {
			expect(this).toBe(globalThis);
			return Promise.resolve(
				new Response(
					JSON.stringify({
						source: "default",
						user_info: { subject: "user-1" },
						principal_resolution: { status: "resolved" },
					}),
					{ status: 200 },
				),
			);
		});

		globalThis.fetch = globalFetch as typeof fetch;

		try {
			const client = createAmagiAuthClient({
				baseUrl: "https://auth.example.com",
				host: AmagiAuthHost.Test,
				persistentStore: createMemoryRecordStore({ prefix: "persistent:" }),
				sessionStore: createMemoryRecordStore({ prefix: "session:" }),
				transport: {
					async execute() {
						return { status: 500, headers: {}, body: null };
					},
				},
				scheduler: {
					setTimeout() {
						return { cancel() {} };
					},
				},
				clock: { now: () => Date.now() },
			});

			await client.ensureReady({
				location: {
					href: "https://app.example.com/auth/token-set/oidc/source/default/callback#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2027-01-01T00%3A05%3A00Z",
					hash: "#access_token=callback-at&id_token=callback-idt&refresh_token=callback-rt&expires_at=2027-01-01T00%3A05%3A00Z",
				},
				history: { replaceState() {} },
			});

			await expect(client.loadBoundUserInfo()).resolves.toEqual({
				source: "default",
				userInfo: { subject: "user-1" },
				principalResolution: { status: "resolved" },
			});
			expect(globalFetch).toHaveBeenCalledOnce();
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	it("adapts browser storage into a record store with take semantics", async () => {
		const bucket: Record<string, unknown> = {};
		const recordStore = createBrowserStorageRecordStore({
			storageArea: {
				async get(key) {
					if (typeof key === "string") {
						return { [key]: bucket[key] };
					}
					return bucket;
				},
				async set(items) {
					Object.assign(bucket, items);
				},
				async remove(keys) {
					for (const key of Array.isArray(keys) ? keys : [keys]) {
						delete bucket[key];
					}
				},
			},
			prefix: "ext:",
		});

		await recordStore.set("auth", "token");
		expect(await recordStore.get("auth")).toBe("token");
		expect(await recordStore.take?.("auth")).toBe("token");
		expect(await recordStore.get("auth")).toBeNull();
	});
});

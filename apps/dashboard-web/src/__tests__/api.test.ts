import { describe, expect, it, vi } from "vitest";

import { createDashboardApiClient } from "../api";

describe("dashboard api client", () => {
	it("sends camelCase dashboard headers and returns profile data", async () => {
		const fetchImpl = vi.fn(
			async (input: URL | RequestInfo, init?: RequestInit) => {
				expect(String(input)).toBe(
					"http://127.0.0.1:7800/api/v1/dashboard/sync-profiles",
				);
				expect(init?.headers).toMatchObject({
					authorization: "Bearer sdk-token",
					"x-amagi-oidc-source": "default",
				});
				return new Response(
					JSON.stringify([
						{
							id: "profile-1",
							name: "Default",
							mode: "manual",
							defaultDirection: "bidirectional",
							conflictPolicy: "manual",
							enabled: true,
							rules: [],
							targets: [],
						},
					]),
					{ status: 200 },
				);
			},
		);

		const client = createDashboardApiClient({
			connection: {
				apiBaseUrl: "http://127.0.0.1:7800",
				oidcSource: "default",
				devBearerToken: "dev-token",
			},
			authorizationHeader: "Bearer sdk-token",
			fetchImpl: fetchImpl as typeof fetch,
		});

		await expect(client.listSyncProfiles()).resolves.toHaveLength(1);
	});

	it("throws a typed dashboard api error for error payloads", async () => {
		const client = createDashboardApiClient({
			connection: {
				apiBaseUrl: "http://127.0.0.1:7800",
				oidcSource: "default",
				devBearerToken: "dev-token",
			},
			fetchImpl: (async () =>
				new Response(
					JSON.stringify({
						code: "unauthenticated",
						message: "missing binding",
						source: null,
					}),
					{ status: 401 },
				)) as typeof fetch,
		});

		await expect(client.listSyncProfiles()).rejects.toMatchObject({
			name: "DashboardApiError",
			code: "unauthenticated",
			message: "missing binding",
		});
	});

	it("falls back to the dev bearer token when no auth helper header is provided", async () => {
		const fetchImpl = vi.fn(
			async (_input: URL | RequestInfo, init?: RequestInit) => {
				expect(init?.headers).toMatchObject({
					authorization: "Bearer dev-token",
					"x-amagi-oidc-source": "default",
				});
				return new Response(JSON.stringify([]), { status: 200 });
			},
		);

		const client = createDashboardApiClient({
			connection: {
				apiBaseUrl: "http://127.0.0.1:7800",
				oidcSource: "default",
				devBearerToken: "dev-token",
			},
			fetchImpl: fetchImpl as typeof fetch,
		});

		await expect(client.listSyncProfiles()).resolves.toEqual([]);
	});
});

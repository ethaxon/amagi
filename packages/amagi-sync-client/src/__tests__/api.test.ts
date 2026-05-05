import { describe, expect, it } from "vitest";

import { AmagiApiError, AmagiSyncApiClient } from "../index";

describe("AmagiSyncApiClient", () => {
	it("normalizes base URL and sends bearer headers", async () => {
		let capturedUrl = "";
		let capturedHeaders = new Headers();
		const client = new AmagiSyncApiClient({
			baseUrl: "https://example.com/api",
			bearerToken: "secret-token",
			oidcSource: "primary",
			fetchImpl: async (input, init) => {
				capturedUrl = String(input);
				capturedHeaders = new Headers(init?.headers);
				return new Response(
					JSON.stringify({
						device: {
							id: "device-1",
							deviceName: "Mac",
							deviceType: "desktop",
							platform: "macos",
							trustLevel: "trusted",
							lastSeenAt: null,
						},
						browserClient: {
							id: "client-1",
							deviceId: "device-1",
							browserFamily: "chrome",
							browserProfileName: "Default",
							extensionInstanceId: "ext-1",
							capabilities: {},
							lastSeenAt: null,
						},
						defaultProfile: {
							id: "profile-1",
							name: "Default",
							mode: "manual",
							defaultDirection: "bidirectional",
							conflictPolicy: "manual",
							enabled: true,
							rules: [],
						},
						cursorSummaries: [],
					}),
					{ status: 201, headers: { "content-type": "application/json" } },
				);
			},
		});

		await client.registerClient({
			device: {
				deviceId: null,
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
			},
			browserClient: {
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
				capabilities: {},
			},
		});

		expect(capturedUrl).toBe(
			"https://example.com/api/v1/sync/clients/register",
		);
		expect(capturedHeaders.get("authorization")).toBe("Bearer secret-token");
		expect(capturedHeaders.get("x-amagi-oidc-source")).toBe("primary");
	});

	it("redacts bearer tokens from structured errors", async () => {
		const client = new AmagiSyncApiClient({
			baseUrl: "https://example.com/",
			bearerToken: "secret-token",
			fetchImpl: async () =>
				new Response(
					JSON.stringify({
						code: "unauthenticated",
						message: "Authorization Bearer secret-token is invalid",
					}),
					{ status: 401, headers: { "content-type": "application/json" } },
				),
		});

		const error = await client
			.startSession({
				browserClientId: "client-1",
				preferredProfileId: null,
				localCapabilitySummary: {},
			})
			.catch((reason: unknown) => reason);

		expect(error).toBeInstanceOf(AmagiApiError);
		expect(error).toMatchObject({ code: "unauthenticated", status: 401 });
		expect((error as Error).message).toMatch(/Bearer \[redacted\]/u);
		expect((error as Error).message).not.toMatch(/secret-token/u);
	});
});

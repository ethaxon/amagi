import { describe, expect, it } from "vitest";

import { ExtensionMessageType } from "../extension/messaging";
import {
	createBackgroundRuntime,
	loadExtensionAuthState,
	requestExtensionStatus,
	requestManualSync,
} from "../extension/runtime";

describe("extension runtime request helpers", () => {
	it("returns a typed error state when status response is missing", async () => {
		await expect(
			requestExtensionStatus({
				runtime: {
					async sendMessage() {
						return undefined;
					},
				},
			}),
		).resolves.toEqual({
			status: "error",
			message: "extension background runtime did not return a status response.",
			previewSummary: null,
		});
	});

	it("returns a typed error state when manual sync response is missing", async () => {
		await expect(
			requestManualSync(
				{
					runtime: {
						async sendMessage() {
							return undefined;
						},
					},
				},
				{ type: ExtensionMessageType.Preview },
			),
		).resolves.toEqual({
			status: "error",
			message:
				"extension background runtime did not return a manual sync response.",
			previewSummary: null,
		});
	});

	it("requires either a token-set session or the advanced fallback before sync", async () => {
		const runtime = createBackgroundRuntime({
			browser: {
				storage: {
					local: {
						async get() {
							return {
								"amagi.extension.config": {
									apiBaseUrl: "http://127.0.0.1:7800",
									oidcSource: "default",
									devBearerToken: "   ",
								},
							};
						},
						async set() {},
					},
				},
			} as Parameters<typeof createBackgroundRuntime>[0]["browser"],
		});

		await expect(
			runtime.handleMessage({ type: ExtensionMessageType.Preview }),
		).resolves.toEqual({
			status: "error",
			message:
				"Log in from the popup or options page before running manual sync, or set the advanced dev bearer fallback.",
			previewSummary: null,
		});
	});

	it("reports unauthenticated auth state when no token-set session exists", async () => {
		await expect(
			loadExtensionAuthState({
				storage: {
					local: {
						async get() {
							return {
								"amagi.extension.config": {
									apiBaseUrl: "http://127.0.0.1:7800",
									oidcSource: "default",
									devBearerToken: "",
								},
							};
						},
						async set() {},
					},
				},
				runtime: {
					getURL(path: string) {
						return `chrome-extension://test/${path}`;
					},
				},
			}),
		).resolves.toEqual({
			status: "unauthenticated",
			message:
				"No token-set session is active. Log in from this extension to enable manual sync.",
			displayName: null,
			subject: null,
			usesDevBearerFallback: false,
		});
	});
});

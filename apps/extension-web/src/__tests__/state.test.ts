import { describe, expect, it } from "vitest";

import { validateExtensionConfig } from "../extension/state";

describe("validateExtensionConfig", () => {
	it("accepts localhost development URLs", () => {
		expect(
			validateExtensionConfig({
				apiBaseUrl: " http://127.0.0.1:7800 ",
				oidcSource: " default ",
				devBearerToken: " dev-token ",
			}),
		).toEqual({
			isValid: true,
			config: {
				apiBaseUrl: "http://127.0.0.1:7800",
				oidcSource: "default",
				devBearerToken: "dev-token",
			},
		});
	});

	it("accepts https self-hosted URLs", () => {
		const result = validateExtensionConfig({
			apiBaseUrl: "https://amagi.example.com",
			oidcSource: "default",
			devBearerToken: "",
		});
		expect(result).toMatchObject({ isValid: true });
	});

	it("rejects non-local http URLs", () => {
		expect(
			validateExtensionConfig({
				apiBaseUrl: "http://example.com:7800",
				oidcSource: "default",
				devBearerToken: "",
			}),
		).toEqual({
			isValid: false,
			message:
				"API Base URL must use https://, http://localhost, or http://127.0.0.1.",
		});
	});

	it("rejects empty oidc source", () => {
		expect(
			validateExtensionConfig({
				apiBaseUrl: "http://localhost:7800",
				oidcSource: "   ",
				devBearerToken: "",
			}),
		).toEqual({
			isValid: false,
			message: "OIDC Source is required.",
		});
	});
});

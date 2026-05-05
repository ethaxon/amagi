import { describe, expect, it } from "vitest";

import {
	DASHBOARD_CONNECTION_STORAGE_KEY,
	loadDashboardConnectionConfig,
	saveDashboardConnectionConfig,
	validateDashboardConnectionConfig,
} from "../state";

describe("dashboard connection state", () => {
	it("normalizes a valid localhost dev config", () => {
		expect(
			validateDashboardConnectionConfig({
				apiBaseUrl: " http://127.0.0.1:7800 ",
				oidcSource: " default ",
				devBearerToken: " token ",
			}),
		).toEqual({
			isValid: true,
			config: {
				apiBaseUrl: "http://127.0.0.1:7800",
				oidcSource: "default",
				devBearerToken: "token",
			},
		});
	});

	it("rejects an invalid remote http dev config", () => {
		expect(
			validateDashboardConnectionConfig({
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

	it("round-trips the dev connection config through storage", () => {
		const storage = new Map<string, string>();
		const storageLike = {
			getItem(key: string) {
				return storage.get(key) ?? null;
			},
			setItem(key: string, value: string) {
				storage.set(key, value);
			},
		};

		saveDashboardConnectionConfig(storageLike, {
			apiBaseUrl: "https://amagi.example.com",
			oidcSource: "default",
			devBearerToken: "dev-token",
		});

		expect(storage.has(DASHBOARD_CONNECTION_STORAGE_KEY)).toBe(true);
		expect(loadDashboardConnectionConfig(storageLike)).toEqual({
			apiBaseUrl: "https://amagi.example.com",
			oidcSource: "default",
			devBearerToken: "dev-token",
		});
	});
});

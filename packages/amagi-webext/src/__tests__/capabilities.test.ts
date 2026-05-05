import { describe, expect, it } from "vitest";

import { detectWebExtCapabilities } from "../index";

describe("detectWebExtCapabilities", () => {
	it("detects full browser capabilities", async () => {
		await expect(
			detectWebExtCapabilities({
				browser: undefined as never,
			} as never),
		).resolves.toMatchObject({
			canReadBookmarks: false,
			canWriteBookmarks: false,
			canUseStorage: false,
			browserFamily: "unknown",
		});
	});

	it("detects bookmarks storage firefox and manifest version", async () => {
		const capabilities = await detectWebExtCapabilities({
			bookmarks: {
				async getTree() {
					return [];
				},
				async create() {
					return { id: "1" };
				},
				async update(id) {
					return { id };
				},
				async move(id) {
					return { id };
				},
				async removeTree() {},
			},
			storage: {
				local: {
					async get() {
						return {};
					},
					async set() {},
				},
			},
			runtime: {
				async getBrowserInfo() {
					return { name: "Firefox" };
				},
				getManifest() {
					return { manifest_version: 2 };
				},
			},
		});

		expect(capabilities).toEqual({
			canReadBookmarks: true,
			canWriteBookmarks: true,
			canUseStorage: true,
			browserFamily: "firefox",
			manifestVersion: 2,
		});
	});
});

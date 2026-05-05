import { describe, expect, it } from "vitest";

import { createWebExtStorage } from "../index";

describe("createWebExtStorage", () => {
	it("round-trips sync state", async () => {
		const bucket: Record<string, unknown> = {};
		const storage = createWebExtStorage({
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
			},
		});

		const initial = await storage.loadState();
		expect(initial.browserClientId).toBeNull();

		await storage.saveState({
			...initial,
			browserClientId: "client-1",
			mappingsByClientExternalId: { local: "server" },
		});

		const restored = await storage.loadState();
		expect(restored.browserClientId).toBe("client-1");
		expect(restored.mappingsByClientExternalId.local).toBe("server");
	});
});

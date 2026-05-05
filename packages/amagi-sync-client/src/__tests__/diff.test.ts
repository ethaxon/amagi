import { describe, expect, it } from "vitest";

import { buildLocalMutations, normalizeLocalTree } from "../index";

describe("buildLocalMutations", () => {
	it("emits create update move and delete baseline", () => {
		const previousTree = normalizeLocalTree([
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
				children: [
					{
						clientExternalId: "mapped",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "Before",
						url: "https://before.example",
						sortKey: "a1",
					},
					{
						clientExternalId: "deleted",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "Delete Me",
						url: "https://delete.example",
						sortKey: "a2",
					},
				],
			},
		]);
		const currentTree = normalizeLocalTree([
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
				children: [
					{
						clientExternalId: "mapped",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "After",
						url: "https://after.example",
						sortKey: "b1",
					},
					{
						clientExternalId: "created",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "Create Me",
						url: "https://create.example",
						sortKey: "c1",
					},
				],
			},
		]);

		const result = buildLocalMutations({
			currentTree,
			previousTree,
			mappingsByClientExternalId: {
				mapped: "server-mapped",
				deleted: "server-deleted",
				root: "server-root",
			},
		});

		expect(result.localMutations.map((mutation) => mutation.op)).toEqual([
			"update",
			"move",
			"create",
			"delete",
		]);
		expect(result.localMutations[2]?.parentServerNodeId).toBe("server-root");
		expect(result.localSnapshotSummary.nodeCount).toBe(3);
	});
});

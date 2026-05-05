import { describe, expect, it } from "vitest";

import { normalizeLocalTree, SYNTHETIC_ROOT_ID } from "../index";

describe("normalizeLocalTree", () => {
	it("builds a flat adjacency list and trims URLs", () => {
		const tree = normalizeLocalTree([
			{
				clientExternalId: "1",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
				children: [
					{
						clientExternalId: "2",
						parentClientExternalId: "1",
						nodeType: "bookmark",
						title: "Example",
						url: "  https://example.com  ",
						sortKey: "a1",
					},
				],
			},
		]);

		expect(tree.rootId).toBe(SYNTHETIC_ROOT_ID);
		expect(tree.rootChildIds).toEqual(["1"]);
		expect(tree.nodes["1"]?.isRoot).toBe(true);
		expect(tree.nodes["2"]?.url).toBe("https://example.com");
		expect(tree.nodes["1"]?.childIds).toEqual(["2"]);
	});
});

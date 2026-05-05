import { describe, expect, it } from "vitest";

import { buildLocalApplyPlan } from "../index";

describe("buildLocalApplyPlan", () => {
	it("orders create update move delete phases for real revision payloads", () => {
		const plan = buildLocalApplyPlan({
			serverOps: [
				{
					revId: "4",
					nodeId: "server-4",
					opType: "node.delete",
					logicalClock: 4,
					payload: { node: { nodeType: "bookmark" } },
					createdAt: "2026-05-05T00:00:00Z",
				},
				{
					revId: "2",
					nodeId: "server-2",
					opType: "node.move",
					logicalClock: 2,
					payload: {
						node: {
							nodeType: "bookmark",
							parentId: "server-stale-parent",
							sortKey: "b1",
						},
						parentId: "server-root",
					},
					createdAt: "2026-05-05T00:00:00Z",
				},
				{
					revId: "1",
					nodeId: "server-1",
					opType: "node.create",
					logicalClock: 1,
					payload: {
						node: {
							nodeType: "folder",
							title: "Folder",
							parentId: "server-root",
							sortKey: "a1",
						},
					},
					createdAt: "2026-05-05T00:00:00Z",
				},
				{
					revId: "3",
					nodeId: "server-3",
					opType: "node.update",
					logicalClock: 3,
					payload: {
						node: {
							nodeType: "bookmark",
							title: "Updated",
							url: "https://example.com",
						},
					},
					createdAt: "2026-05-05T00:00:00Z",
				},
			],
			mappingsByClientExternalId: {
				root: "server-root",
				bookmark: "server-3",
			},
		});

		expect(plan.map((entry) => entry.kind)).toEqual([
			"create",
			"update",
			"move",
			"delete",
		]);
		expect(plan[0]?.phase).toBe(1);
		expect(plan[3]?.phase).toBe(4);
		expect(plan[0]).toMatchObject({
			kind: "create",
			parentClientExternalId: "root",
			title: "Folder",
			sortKey: "a1",
		});
		expect(plan[1]).toMatchObject({
			kind: "update",
			title: "Updated",
			url: "https://example.com",
		});
		expect(plan[2]).toMatchObject({
			kind: "move",
			parentClientExternalId: "root",
			sortKey: "b1",
		});
	});
});

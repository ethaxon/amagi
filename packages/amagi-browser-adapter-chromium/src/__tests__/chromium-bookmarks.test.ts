import type { LocalApplyOp } from "@ethaxon/amagi-sync-client";
import { describe, expect, it } from "vitest";

import { createChromiumBookmarkAdapter } from "../index";

describe("createChromiumBookmarkAdapter", () => {
	it("loads bookmark tree without the browser root wrapper", async () => {
		const adapter = createChromiumBookmarkAdapter({
			bookmarks: {
				async getTree() {
					return [
						{
							id: "0",
							children: [
								{
									id: "1",
									title: "Bookmarks Bar",
									children: [
										{
											id: "2",
											title: "Example",
											url: "https://example.com",
											index: 0,
										},
									],
								},
							],
						},
					];
				},
				async create() {
					throw new Error("not used");
				},
				async update() {
					throw new Error("not used");
				},
				async move() {
					throw new Error("not used");
				},
				async removeTree() {
					throw new Error("not used");
				},
			},
			storage: {
				local: {
					async get() {
						return {};
					},
					async set() {},
				},
			},
		});

		const forest = await adapter.loadTree();
		expect(forest).toHaveLength(1);
		expect(forest[0]?.clientExternalId).toBe("1");
		expect(forest[0]?.children?.[0]?.clientExternalId).toBe("2");
	});

	it("applies create update move delete ops through bookmark APIs", async () => {
		const calls: string[] = [];
		const adapter = createChromiumBookmarkAdapter({
			bookmarks: {
				async getTree() {
					return [];
				},
				async create(details) {
					calls.push(
						`create:${details.parentId ?? "root"}:${details.title ?? ""}`,
					);
					return { id: "created-1", title: details.title };
				},
				async update(id, changes) {
					calls.push(`update:${id}:${changes.title ?? ""}`);
					return { id, title: changes.title };
				},
				async move(id, destination) {
					calls.push(`move:${id}:${destination.parentId ?? "root"}`);
					return { id, parentId: destination.parentId };
				},
				async removeTree(id) {
					calls.push(`delete:${id}`);
				},
			},
			storage: {
				local: {
					async get() {
						return {};
					},
					async set() {},
				},
			},
		});

		await adapter.applyLocalPlan([
			{
				kind: "create",
				phase: 1,
				clientExternalId: "client-new",
				parentClientExternalId: "1",
				nodeType: "bookmark",
				title: "Created",
				url: "https://created.example",
				sortKey: "2",
			},
			{
				kind: "update",
				phase: 2,
				clientExternalId: "2",
				title: "Updated",
				url: "https://updated.example",
			},
			{
				kind: "move",
				phase: 3,
				clientExternalId: "2",
				parentClientExternalId: "3",
				sortKey: "1",
			},
			{
				kind: "delete",
				phase: 4,
				clientExternalId: "4",
			},
		] satisfies LocalApplyOp[]);

		expect(calls).toEqual([
			"create:1:Created",
			"update:2:Updated",
			"move:2:3",
			"delete:4",
		]);
	});
});

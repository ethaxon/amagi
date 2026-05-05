import type {
	LocalApplyCreatedMapping,
	LocalApplyOp,
	LocalBookmarkNode,
	SyncAdapter,
} from "@ethaxon/amagi-sync-client";

import { detectWebExtCapabilities } from "./capabilities";
import type { BrowserBookmarkNodeLike, BrowserLike } from "./types";

export function createWebExtBookmarkAdapter(options: {
	browser: BrowserLike;
}): SyncAdapter {
	return {
		async getCapabilities() {
			const capabilities = await detectWebExtCapabilities(options.browser);
			return {
				canReadBookmarks: capabilities.canReadBookmarks,
				canWriteBookmarks: capabilities.canWriteBookmarks,
			};
		},
		async loadTree() {
			const [root] = (await options.browser.bookmarks?.getTree?.()) ?? [];
			if (!root) {
				return [];
			}
			return (root.children ?? []).map((node, index) =>
				toLocalNode(node, null, index),
			);
		},
		async applyLocalPlan(plan: LocalApplyOp[]) {
			const createdMappings: LocalApplyCreatedMapping[] = [];
			for (const op of plan) {
				const createdMapping = await applyOp(options.browser, op);
				if (createdMapping) {
					createdMappings.push(createdMapping);
				}
			}
			return { createdMappings };
		},
	};
}

function toLocalNode(
	node: BrowserBookmarkNodeLike,
	parentClientExternalId: string | null,
	index: number,
): LocalBookmarkNode {
	const children = node.children?.map((child, childIndex) =>
		toLocalNode(child, node.id, childIndex),
	);
	return {
		clientExternalId: node.id,
		parentClientExternalId,
		nodeType: children ? "folder" : node.url ? "bookmark" : "separator",
		title: node.title ?? "",
		url: node.url ?? null,
		sortKey: String(node.index ?? index),
		...(children ? { children } : {}),
	};
}

async function applyOp(
	browserLike: BrowserLike,
	op: LocalApplyOp,
): Promise<LocalApplyCreatedMapping | null> {
	const bookmarks = browserLike.bookmarks;
	if (!bookmarks) {
		throw new Error("browser.bookmarks API is unavailable");
	}
	switch (op.kind) {
		case "create":
			return bookmarks
				.create({
					parentId: op.parentClientExternalId ?? undefined,
					index: toIndex(op.sortKey),
					title: op.title,
					...(op.nodeType === "bookmark" && op.url ? { url: op.url } : {}),
				})
				.then((node) => ({
					serverNodeId: op.serverNodeId,
					clientExternalId: node.id,
				}));
		case "update":
			await bookmarks.update(op.clientExternalId, {
				title: op.title,
				url: op.url ?? undefined,
			});
			return null;
		case "move":
			await bookmarks.move(op.clientExternalId, {
				parentId: op.parentClientExternalId ?? undefined,
				index: toIndex(op.sortKey),
			});
			return null;
		case "delete":
			if (bookmarks.removeTree) {
				await bookmarks.removeTree(op.clientExternalId);
				return null;
			}
			if (bookmarks.remove) {
				await bookmarks.remove(op.clientExternalId);
				return null;
			}
			throw new Error("browser.bookmarks remove APIs are unavailable");
	}
}

function toIndex(sortKey: string | null): number | undefined {
	if (sortKey === null) {
		return undefined;
	}
	const parsed = Number.parseInt(sortKey, 10);
	return Number.isFinite(parsed) ? parsed : undefined;
}

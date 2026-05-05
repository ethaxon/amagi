import type {
	LocalApplyOp,
	LocalBookmarkNode,
	SyncAdapter,
} from "@ethaxon/amagi-sync-client";

import type { ChromiumBookmarkNode, ChromiumLike } from "./types";

export function createChromiumBookmarkAdapter(
	chromeLike: ChromiumLike,
): SyncAdapter {
	return {
		getCapabilities() {
			return {
				canReadBookmarks: true,
				canWriteBookmarks: true,
			};
		},
		async loadTree() {
			const [root] = await chromeLike.bookmarks.getTree();
			if (!root) {
				return [];
			}
			return (root.children ?? []).map((node, index) =>
				toLocalNode(node, null, index),
			);
		},
		async applyLocalPlan(plan) {
			for (const op of plan) {
				await applyOp(chromeLike, op);
			}
		},
	};
}

function toLocalNode(
	node: ChromiumBookmarkNode,
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
	chromeLike: ChromiumLike,
	op: LocalApplyOp,
): Promise<void> {
	switch (op.kind) {
		case "create":
			await chromeLike.bookmarks.create({
				parentId: op.parentClientExternalId ?? undefined,
				index: toIndex(op.sortKey),
				title: op.title,
				...(op.nodeType === "bookmark" && op.url ? { url: op.url } : {}),
			});
			return;
		case "update":
			await chromeLike.bookmarks.update(op.clientExternalId, {
				title: op.title,
				url: op.url ?? undefined,
			});
			return;
		case "move":
			await chromeLike.bookmarks.move(op.clientExternalId, {
				parentId: op.parentClientExternalId ?? undefined,
				index: toIndex(op.sortKey),
			});
			return;
		case "delete":
			if (chromeLike.bookmarks.remove) {
				await chromeLike.bookmarks.remove(op.clientExternalId);
				return;
			}
			await chromeLike.bookmarks.removeTree(op.clientExternalId);
			return;
	}
}

function toIndex(sortKey: string | null): number | undefined {
	if (sortKey === null) {
		return undefined;
	}
	const parsed = Number.parseInt(sortKey, 10);
	return Number.isFinite(parsed) ? parsed : undefined;
}

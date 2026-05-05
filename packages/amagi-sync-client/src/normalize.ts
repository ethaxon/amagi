import { SyncClientError } from "./errors";
import {
	type LocalBookmarkNode,
	type LocalBookmarkTree,
	type NormalizedLocalBookmarkNode,
	normalizeUrl,
	SYNTHETIC_ROOT_ID,
} from "./local-tree";

export function normalizeLocalTree(
	forest: LocalBookmarkNode[],
): LocalBookmarkTree {
	const nodes: Record<string, NormalizedLocalBookmarkNode> = {
		[SYNTHETIC_ROOT_ID]: {
			clientExternalId: SYNTHETIC_ROOT_ID,
			parentClientExternalId: null,
			nodeType: "folder",
			title: "__amagi_local_root__",
			url: null,
			sortKey: null,
			childIds: [],
			depth: 0,
			isRoot: false,
			isSyntheticRoot: true,
		},
	};
	const orderedIds: string[] = [SYNTHETIC_ROOT_ID];

	for (const rootNode of forest) {
		visitNode(rootNode, nodes, orderedIds, 1, true);
		nodes[SYNTHETIC_ROOT_ID].childIds.push(rootNode.clientExternalId);
	}

	return {
		rootId: SYNTHETIC_ROOT_ID,
		rootChildIds: [...nodes[SYNTHETIC_ROOT_ID].childIds],
		orderedIds,
		nodes,
	};
}

function visitNode(
	node: LocalBookmarkNode,
	nodes: Record<string, NormalizedLocalBookmarkNode>,
	orderedIds: string[],
	depth: number,
	isRoot: boolean,
): void {
	const clientExternalId = node.clientExternalId.trim();
	if (!clientExternalId) {
		throw new SyncClientError(
			"local bookmark node requires a non-empty clientExternalId",
		);
	}
	if (nodes[clientExternalId]) {
		throw new SyncClientError(
			`duplicate local bookmark node clientExternalId: ${clientExternalId}`,
		);
	}

	const normalized: NormalizedLocalBookmarkNode = {
		clientExternalId,
		parentClientExternalId: isRoot
			? SYNTHETIC_ROOT_ID
			: node.parentClientExternalId?.trim() || null,
		nodeType: node.nodeType,
		title: node.title.trim(),
		url: normalizeUrl(node.url),
		sortKey: node.sortKey?.trim() || null,
		childIds: [],
		depth,
		isRoot,
		isSyntheticRoot: false,
	};

	nodes[clientExternalId] = normalized;
	orderedIds.push(clientExternalId);

	for (const child of node.children ?? []) {
		normalized.childIds.push(child.clientExternalId);
		visitNode(child, nodes, orderedIds, depth + 1, false);
	}
	if (node.nodeType !== "folder" && normalized.childIds.length > 0) {
		throw new SyncClientError(
			`non-folder node ${clientExternalId} cannot carry children in local tree normalization`,
		);
	}
}

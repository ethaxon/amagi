import { SyncClientError } from "./errors";
import type { LocalBookmarkTree } from "./local-tree";
import type { LocalMutationInput } from "./types";

export interface DiffInput {
	currentTree: LocalBookmarkTree;
	previousTree: LocalBookmarkTree | null;
	mappingsByClientExternalId: Record<string, string>;
}

export interface LocalSnapshotSummary {
	rootHash: string;
	nodeCount: number;
}

export interface DiffResult {
	localMutations: LocalMutationInput[];
	localSnapshotSummary: LocalSnapshotSummary;
}

export function buildLocalMutations(input: DiffInput): DiffResult {
	const localMutations: LocalMutationInput[] = [];
	const previousTree = input.previousTree;

	for (const clientExternalId of input.currentTree.orderedIds) {
		const currentNode = input.currentTree.nodes[clientExternalId];
		if (!currentNode || currentNode.isSyntheticRoot || currentNode.isRoot) {
			continue;
		}
		const serverNodeId =
			input.mappingsByClientExternalId[clientExternalId] ?? null;
		const previousNode = previousTree?.nodes[clientExternalId] ?? null;
		if (!serverNodeId) {
			localMutations.push({
				clientMutationId: mutationId("create", clientExternalId),
				op: "create",
				serverNodeId: null,
				clientExternalId,
				parentServerNodeId: currentNode.parentClientExternalId
					? (input.mappingsByClientExternalId[
							currentNode.parentClientExternalId
						] ?? null)
					: null,
				parentClientExternalId:
					currentNode.parentClientExternalId &&
					!input.mappingsByClientExternalId[currentNode.parentClientExternalId]
						? currentNode.parentClientExternalId
						: null,
				nodeType: currentNode.nodeType,
				title: currentNode.title,
				url: currentNode.url,
				sortKey: currentNode.sortKey,
			});
			continue;
		}
		if (!previousNode) {
			continue;
		}
		if (
			currentNode.title !== previousNode.title ||
			currentNode.url !== previousNode.url
		) {
			localMutations.push({
				clientMutationId: mutationId("update", clientExternalId),
				op: "update",
				serverNodeId,
				clientExternalId,
				parentServerNodeId: null,
				parentClientExternalId: null,
				nodeType: currentNode.nodeType,
				title: currentNode.title,
				url: currentNode.url,
				sortKey: currentNode.sortKey,
			});
		}
		if (
			currentNode.parentClientExternalId !==
				previousNode.parentClientExternalId ||
			currentNode.sortKey !== previousNode.sortKey
		) {
			localMutations.push({
				clientMutationId: mutationId("move", clientExternalId),
				op: "move",
				serverNodeId,
				clientExternalId,
				parentServerNodeId: currentNode.parentClientExternalId
					? (input.mappingsByClientExternalId[
							currentNode.parentClientExternalId
						] ?? null)
					: null,
				parentClientExternalId:
					currentNode.parentClientExternalId &&
					!input.mappingsByClientExternalId[currentNode.parentClientExternalId]
						? currentNode.parentClientExternalId
						: null,
				nodeType: currentNode.nodeType,
				title: currentNode.title,
				url: currentNode.url,
				sortKey: currentNode.sortKey,
			});
		}
	}

	if (previousTree) {
		for (const clientExternalId of previousTree.orderedIds) {
			const previousNode = previousTree.nodes[clientExternalId];
			if (
				!previousNode ||
				previousNode.isSyntheticRoot ||
				previousNode.isRoot
			) {
				continue;
			}
			const serverNodeId = input.mappingsByClientExternalId[clientExternalId];
			if (!serverNodeId) {
				continue;
			}
			if (!input.currentTree.nodes[clientExternalId]) {
				localMutations.push({
					clientMutationId: mutationId("delete", clientExternalId),
					op: "delete",
					serverNodeId,
					clientExternalId,
					parentServerNodeId: null,
					parentClientExternalId: null,
					nodeType: previousNode.nodeType,
					title: null,
					url: null,
					sortKey: null,
				});
			}
		}
	}

	return {
		localMutations,
		localSnapshotSummary: createSnapshotSummary(input.currentTree),
	};
}

export function createSnapshotSummary(
	tree: LocalBookmarkTree,
): LocalSnapshotSummary {
	const stableNodes = tree.orderedIds
		.filter((id) => id !== tree.rootId)
		.map((id) => {
			const node = tree.nodes[id];
			if (!node) {
				throw new SyncClientError(
					`local tree is missing normalized node ${id}`,
				);
			}
			return {
				id: node.clientExternalId,
				parentId: node.parentClientExternalId,
				nodeType: node.nodeType,
				title: node.title,
				url: node.url,
				sortKey: node.sortKey,
				childIds: node.childIds,
				isRoot: node.isRoot,
			};
		});
	const rootHash = createHash(JSON.stringify(stableNodes));
	return {
		rootHash,
		nodeCount: stableNodes.length,
	};
}

function mutationId(op: string, clientExternalId: string): string {
	return `${op}:${clientExternalId}`;
}

function createHash(value: string): string {
	let hash = 5381;
	for (let index = 0; index < value.length; index += 1) {
		hash = (hash * 33) ^ value.charCodeAt(index);
	}
	return `djb2-${(hash >>> 0).toString(16)}`;
}

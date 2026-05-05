import { SyncClientError } from "./errors";
import type {
	LocalNodeType,
	NodeClientMappingView,
	ServerOpView,
} from "./types";

export type LocalApplyOp =
	| {
			kind: "create";
			phase: 1;
			clientExternalId: string;
			parentClientExternalId: string | null;
			nodeType: LocalNodeType;
			title: string;
			url: string | null;
			sortKey: string | null;
	  }
	| {
			kind: "update";
			phase: 2;
			clientExternalId: string;
			title: string;
			url: string | null;
	  }
	| {
			kind: "move";
			phase: 3;
			clientExternalId: string;
			parentClientExternalId: string | null;
			sortKey: string | null;
	  }
	| {
			kind: "delete";
			phase: 4;
			clientExternalId: string;
	  };

export interface ApplyPlanContext {
	serverOps: ServerOpView[];
	mappingsByClientExternalId: Record<string, string>;
	createdMappings?: NodeClientMappingView[];
}

export function buildLocalApplyPlan(context: ApplyPlanContext): LocalApplyOp[] {
	const reverseMapping = toReverseMapping(
		context.mappingsByClientExternalId,
		context.createdMappings ?? [],
	);
	const ops = context.serverOps.map((serverOp) =>
		toLocalApplyOp(serverOp, reverseMapping),
	);
	return ops.sort((left, right) => left.phase - right.phase);
}

function toReverseMapping(
	mappingsByClientExternalId: Record<string, string>,
	createdMappings: NodeClientMappingView[],
): Record<string, string> {
	const reverseMapping: Record<string, string> = {};
	for (const [clientExternalId, serverNodeId] of Object.entries(
		mappingsByClientExternalId,
	)) {
		reverseMapping[serverNodeId] = clientExternalId;
	}
	for (const mapping of createdMappings) {
		reverseMapping[mapping.serverNodeId] = mapping.clientExternalId;
	}
	return reverseMapping;
}

function toLocalApplyOp(
	serverOp: ServerOpView,
	reverseMapping: Record<string, string>,
): LocalApplyOp {
	const payload = serverOp.payload;
	const nodePayload = readObject(payload.node);
	const nodeType = readNodeType(nodePayload?.nodeType ?? payload.nodeType);
	const clientExternalId =
		reverseMapping[serverOp.nodeId] ?? `server:${serverOp.nodeId}`;
	const parentServerNodeId =
		readString(payload.parentId) ??
		readString(payload.parentNodeId) ??
		readString(nodePayload?.parentId) ??
		null;
	const parentClientExternalId = parentServerNodeId
		? (reverseMapping[parentServerNodeId] ?? `server:${parentServerNodeId}`)
		: null;

	switch (serverOp.opType) {
		case "node.create":
		case "node.restore":
			return {
				kind: "create",
				phase: 1,
				clientExternalId,
				parentClientExternalId,
				nodeType,
				title: readString(nodePayload?.title ?? payload.title) ?? "",
				url: readNullableString(nodePayload?.url ?? payload.url),
				sortKey: readNullableString(nodePayload?.sortKey ?? payload.sortKey),
			};
		case "node.update":
			return {
				kind: "update",
				phase: 2,
				clientExternalId,
				title: readString(nodePayload?.title ?? payload.title) ?? "",
				url: readNullableString(nodePayload?.url ?? payload.url),
			};
		case "node.move":
			return {
				kind: "move",
				phase: 3,
				clientExternalId,
				parentClientExternalId,
				sortKey: readNullableString(nodePayload?.sortKey ?? payload.sortKey),
			};
		case "node.delete":
			return {
				kind: "delete",
				phase: 4,
				clientExternalId,
			};
		default:
			throw new SyncClientError(
				`unsupported server op type: ${serverOp.opType}`,
			);
	}
}

function readNodeType(value: unknown): LocalNodeType {
	if (value === "folder" || value === "bookmark" || value === "separator") {
		return value;
	}
	throw new SyncClientError("server op payload is missing a valid nodeType");
}

function readObject(value: unknown): Record<string, unknown> | null {
	return typeof value === "object" && value !== null
		? (value as Record<string, unknown>)
		: null;
}

function readString(value: unknown): string | null {
	return typeof value === "string" ? value : null;
}

function readNullableString(value: unknown): string | null {
	if (value === null || value === undefined) {
		return null;
	}
	if (typeof value === "string") {
		return value;
	}
	throw new SyncClientError(
		"server op payload contains an invalid string field",
	);
}

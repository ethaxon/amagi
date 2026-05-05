import type { LocalNodeType } from "./types";

export interface LocalBookmarkNode {
	clientExternalId: string;
	parentClientExternalId: string | null;
	nodeType: LocalNodeType;
	title: string;
	url: string | null;
	sortKey: string | null;
	children?: LocalBookmarkNode[];
}

export interface NormalizedLocalBookmarkNode {
	clientExternalId: string;
	parentClientExternalId: string | null;
	nodeType: LocalNodeType;
	title: string;
	url: string | null;
	sortKey: string | null;
	childIds: string[];
	depth: number;
	isRoot: boolean;
	isSyntheticRoot: boolean;
}

export interface LocalBookmarkTree {
	rootId: string;
	rootChildIds: string[];
	orderedIds: string[];
	nodes: Record<string, NormalizedLocalBookmarkNode>;
}

export const SYNTHETIC_ROOT_ID = "__amagi_local_root__";

export function normalizeUrl(url: string | null | undefined): string | null {
	if (typeof url !== "string") {
		return null;
	}
	const trimmed = url.trim();
	return trimmed ? trimmed : null;
}

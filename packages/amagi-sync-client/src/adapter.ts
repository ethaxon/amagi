import type { LocalApplyOp } from "./apply-plan";
import type { LocalBookmarkNode } from "./local-tree";

export interface LocalApplyCreatedMapping {
	serverNodeId: string;
	clientExternalId: string;
}

export interface LocalApplyResult {
	createdMappings: LocalApplyCreatedMapping[];
}

export interface SyncAdapterCapabilities {
	canReadBookmarks: boolean;
	canWriteBookmarks: boolean;
}

export interface SyncAdapter {
	getCapabilities(): Promise<SyncAdapterCapabilities> | SyncAdapterCapabilities;
	loadTree(): Promise<LocalBookmarkNode[]>;
	applyLocalPlan(plan: LocalApplyOp[]): Promise<LocalApplyResult>;
}

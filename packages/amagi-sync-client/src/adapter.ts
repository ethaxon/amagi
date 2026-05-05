import type { LocalApplyOp } from "./apply-plan";
import type { LocalBookmarkNode } from "./local-tree";

export interface SyncAdapterCapabilities {
	canReadBookmarks: boolean;
	canWriteBookmarks: boolean;
}

export interface SyncAdapter {
	getCapabilities(): Promise<SyncAdapterCapabilities> | SyncAdapterCapabilities;
	loadTree(): Promise<LocalBookmarkNode[]>;
	applyLocalPlan(plan: LocalApplyOp[]): Promise<void>;
}

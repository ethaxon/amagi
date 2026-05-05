import type { ManualSyncResult } from "@ethaxon/amagi-sync-client";

import type { ExtensionViewState } from "./state";

export const ExtensionMessageType = {
	Preview: "amagi.sync.preview",
	Apply: "amagi.sync.apply",
	Status: "amagi.sync.status",
} as const;

export type ExtensionMessageType =
	(typeof ExtensionMessageType)[keyof typeof ExtensionMessageType];

export type ExtensionMessage =
	| { type: typeof ExtensionMessageType.Preview }
	| { type: typeof ExtensionMessageType.Apply }
	| { type: typeof ExtensionMessageType.Status };

export type ExtensionMessageResponse = ExtensionViewState | ManualSyncResult;

export function isExtensionMessage(value: unknown): value is ExtensionMessage {
	return (
		typeof value === "object" &&
		value !== null &&
		"type" in value &&
		(value.type === ExtensionMessageType.Preview ||
			value.type === ExtensionMessageType.Apply ||
			value.type === ExtensionMessageType.Status)
	);
}

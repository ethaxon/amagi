export const SyncProfileMode = {
	Manual: "manual",
} as const;

export type SyncProfileMode =
	(typeof SyncProfileMode)[keyof typeof SyncProfileMode];

export const SYNC_PROFILE_MODES = [SyncProfileMode.Manual] as const;

export const DefaultDirection = {
	Pull: "pull",
	Push: "push",
	Bidirectional: "bidirectional",
} as const;

export type DefaultDirection =
	(typeof DefaultDirection)[keyof typeof DefaultDirection];

export const DEFAULT_DIRECTIONS = [
	DefaultDirection.Pull,
	DefaultDirection.Push,
	DefaultDirection.Bidirectional,
] as const;

export const ConflictPolicy = {
	Manual: "manual",
} as const;

export type ConflictPolicy =
	(typeof ConflictPolicy)[keyof typeof ConflictPolicy];

export const CONFLICT_POLICIES = [ConflictPolicy.Manual] as const;

export const RuleAction = {
	Include: "include",
	Exclude: "exclude",
	Readonly: "readonly",
} as const;

export type RuleAction = (typeof RuleAction)[keyof typeof RuleAction];

export const RULE_ACTIONS = [
	RuleAction.Include,
	RuleAction.Exclude,
	RuleAction.Readonly,
] as const;

export const RuleMatcherType = {
	LibraryKind: "library_kind",
	FolderId: "folder_id",
	FolderPath: "folder_path",
	Tag: "tag",
} as const;

export type RuleMatcherType =
	(typeof RuleMatcherType)[keyof typeof RuleMatcherType];

export const RULE_MATCHER_TYPES = [
	RuleMatcherType.LibraryKind,
	RuleMatcherType.FolderId,
	RuleMatcherType.FolderPath,
	RuleMatcherType.Tag,
] as const;

export const LibraryKind = {
	Normal: "normal",
	Vault: "vault",
} as const;

export type LibraryKind = (typeof LibraryKind)[keyof typeof LibraryKind];

export const LIBRARY_KINDS = [LibraryKind.Normal, LibraryKind.Vault] as const;

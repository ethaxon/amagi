import { describe, expect, it } from "vitest";

import {
	type CursorAckRequest,
	createMemorySyncStorage,
	type FeedRequest,
	type LocalApplyOp,
	type LocalBookmarkNode,
	normalizeLocalTree,
	runManualSync,
	type SyncAdapter,
	type SyncPreviewRequest,
	type SyncPreviewResponse,
} from "../index";

class FakeAdapter implements SyncAdapter {
	public readonly calls: string[] = [];
	private tree: LocalBookmarkNode[];
	private readonly failOnApply: boolean;

	public constructor(tree: LocalBookmarkNode[], failOnApply = false) {
		this.tree = structuredClone(tree);
		this.failOnApply = failOnApply;
	}

	public getCapabilities() {
		this.calls.push("capabilities");
		return {
			canReadBookmarks: true,
			canWriteBookmarks: true,
		};
	}

	public async loadTree() {
		this.calls.push("loadTree");
		return structuredClone(this.tree);
	}

	public async applyLocalPlan(plan: LocalApplyOp[]) {
		this.calls.push(`apply:${plan.length}`);
		if (this.failOnApply) {
			throw new Error("adapter apply failed");
		}
		return { createdMappings: [] };
	}
}

describe("runManualSync", () => {
	it("completes register session feed preview apply adapter apply and ack", async () => {
		const calls: string[] = [];
		const adapter = new FakeAdapter([
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
				children: [
					{
						clientExternalId: "local-1",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "Example",
						url: "https://example.com",
						sortKey: "a1",
					},
				],
			},
		]);
		const storage = createMemorySyncStorage();
		const preview: SyncPreviewResponse = {
			previewId: "preview-1",
			expiresAt: "2026-05-05T00:10:00Z",
			summary: {
				serverToLocal: 0,
				localToServerAccepted: 1,
				conflicts: 0,
			},
			serverOps: [],
			acceptedLocalMutations: [],
			conflicts: [],
		};
		const api = {
			async registerClient() {
				calls.push("register");
				return {
					device: {
						id: "device-1",
						deviceName: "Mac",
						deviceType: "desktop",
						platform: "macos",
						trustLevel: "trusted",
						lastSeenAt: null,
					},
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					defaultProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					cursorSummaries: [],
				};
			},
			async startSession() {
				calls.push("session");
				return {
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					selectedProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					availableProfiles: [],
					libraries: [
						{
							id: "library-1",
							name: "Default",
							kind: "normal",
							projection: "include",
							currentRevisionClock: 1,
						},
					],
					cursors: [],
					serverTime: "2026-05-05T00:00:00Z",
				};
			},
			async feed() {
				calls.push("feed");
				return {
					browserClientId: "client-1",
					libraryId: "library-1",
					fromClock: 0,
					toClock: 1,
					currentClock: 1,
					serverOps: [],
					nextCursor: null,
				};
			},
			async preview() {
				calls.push("preview");
				return preview;
			},
			async apply() {
				calls.push("apply");
				return {
					applied: true,
					newClock: 2,
					serverOpsToApplyLocally: [],
					createdMappings: [
						{
							browserClientId: "client-1",
							serverNodeId: "server-local-1",
							clientExternalId: "local-1",
						},
					],
					conflicts: [],
				};
			},
			async ackCursor() {
				calls.push("ack");
				return {
					cursor: {
						browserClientId: "client-1",
						libraryId: "library-1",
						lastAppliedClock: 2,
						lastAckRevId: null,
						lastSyncAt: null,
					},
				};
			},
		};

		const result = await runManualSync({
			api,
			adapter,
			storage,
			auth: {
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
			},
			confirmApply: () => true,
		});

		expect(result.status).toBe("synced");
		expect(calls).toEqual([
			"register",
			"session",
			"feed",
			"preview",
			"apply",
			"ack",
		]);
		expect(adapter.calls).toEqual([
			"capabilities",
			"loadTree",
			"apply:0",
			"loadTree",
		]);
		const savedState = await storage.loadState();
		expect(savedState.browserClientId).toBe("client-1");
		expect(savedState.cursorsByLibraryId["library-1"]?.lastAppliedClock).toBe(
			2,
		);
		expect(savedState.mappingsByClientExternalId["local-1"]).toBe(
			"server-local-1",
		);
		expect(savedState.lastKnownTree?.rootId).toBe(
			normalizeLocalTree(await adapter.loadTree()).rootId,
		);
	});

	it("stores pending preview when conflicts exist", async () => {
		const calls: string[] = [];
		const adapter = new FakeAdapter([
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
			},
		]);
		const storage = createMemorySyncStorage({
			browserClientId: "client-1",
			selectedProfileId: "profile-1",
		});
		const api = {
			async registerClient() {
				throw new Error("register should not be called");
			},
			async startSession() {
				calls.push("session");
				return {
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					selectedProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					availableProfiles: [],
					libraries: [
						{
							id: "library-1",
							name: "Default",
							kind: "normal",
							projection: "include",
							currentRevisionClock: 1,
						},
					],
					cursors: [],
					serverTime: "2026-05-05T00:00:00Z",
				};
			},
			async feed() {
				calls.push("feed");
				return {
					browserClientId: "client-1",
					libraryId: "library-1",
					fromClock: 0,
					toClock: 1,
					currentClock: 1,
					serverOps: [],
					nextCursor: null,
				};
			},
			async preview() {
				calls.push("preview");
				return {
					previewId: "preview-1",
					expiresAt: "2026-05-05T00:10:00Z",
					summary: {
						serverToLocal: 0,
						localToServerAccepted: 0,
						conflicts: 1,
					},
					serverOps: [],
					acceptedLocalMutations: [],
					conflicts: [
						{
							conflictType: "mapping_missing",
							summary: "conflict",
							details: {},
						},
					],
				};
			},
			async apply() {
				throw new Error("apply should not be called");
			},
			async ackCursor() {
				throw new Error("ack should not be called");
			},
		};

		const result = await runManualSync({
			api,
			adapter,
			storage,
			auth: {
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
			},
			confirmApply: () => true,
		});

		expect(result.status).toBe("needs-user-resolution");
		expect(calls).toEqual(["session", "feed", "preview"]);
		expect(adapter.calls).toEqual(["capabilities", "loadTree"]);
		expect((await storage.loadState()).pendingPreview?.preview.previewId).toBe(
			"preview-1",
		);
	});

	it("stores recovery state when adapter apply fails and does not ack", async () => {
		const calls: string[] = [];
		const adapter = new FakeAdapter(
			[
				{
					clientExternalId: "root",
					parentClientExternalId: null,
					nodeType: "folder",
					title: "Bookmarks Bar",
					url: null,
					sortKey: "a",
				},
			],
			true,
		);
		const storage = createMemorySyncStorage({
			browserClientId: "client-1",
			selectedProfileId: "profile-1",
		});
		const api = {
			async registerClient() {
				throw new Error("register should not be called");
			},
			async startSession() {
				calls.push("session");
				return {
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					selectedProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					availableProfiles: [],
					libraries: [
						{
							id: "library-1",
							name: "Default",
							kind: "normal",
							projection: "include",
							currentRevisionClock: 1,
						},
					],
					cursors: [],
					serverTime: "2026-05-05T00:00:00Z",
				};
			},
			async feed() {
				calls.push("feed");
				return {
					browserClientId: "client-1",
					libraryId: "library-1",
					fromClock: 0,
					toClock: 1,
					currentClock: 1,
					serverOps: [],
					nextCursor: null,
				};
			},
			async preview() {
				calls.push("preview");
				return {
					previewId: "preview-1",
					expiresAt: "2026-05-05T00:10:00Z",
					summary: {
						serverToLocal: 0,
						localToServerAccepted: 0,
						conflicts: 0,
					},
					serverOps: [],
					acceptedLocalMutations: [],
					conflicts: [],
				};
			},
			async apply() {
				calls.push("apply");
				return {
					applied: true,
					newClock: 2,
					serverOpsToApplyLocally: [],
					createdMappings: [],
					conflicts: [],
				};
			},
			async ackCursor() {
				calls.push("ack");
				return {
					cursor: {
						browserClientId: "client-1",
						libraryId: "library-1",
						lastAppliedClock: 2,
						lastAckRevId: null,
						lastSyncAt: null,
					},
				};
			},
		};

		const result = await runManualSync({
			api,
			adapter,
			storage,
			auth: {
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
			},
			confirmApply: () => true,
		});

		expect(result.status).toBe("recovery-required");
		expect(calls).toEqual(["session", "feed", "preview", "apply"]);
		expect(adapter.calls).toEqual(["capabilities", "loadTree", "apply:0"]);
		expect((await storage.loadState()).pendingRecovery?.previewId).toBe(
			"preview-1",
		);
	});

	it("applies pulled server ops before ack when cursor lags and there are no local mutations", async () => {
		const timeline: string[] = [];
		const initialTree: LocalBookmarkNode[] = [
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
			},
		];
		const storage = createMemorySyncStorage({
			browserClientId: "client-1",
			selectedProfileId: "profile-1",
			lastKnownTree: normalizeLocalTree(initialTree),
			mappingsByClientExternalId: { root: "server-root" },
			cursorsByLibraryId: {
				"library-1": {
					browserClientId: "client-1",
					libraryId: "library-1",
					lastAppliedClock: 0,
					lastAckRevId: null,
					lastSyncAt: null,
				},
			},
		});
		const adapter: SyncAdapter = {
			getCapabilities() {
				timeline.push("capabilities");
				return {
					canReadBookmarks: true,
					canWriteBookmarks: true,
				};
			},
			async loadTree() {
				timeline.push("loadTree");
				return structuredClone(initialTree);
			},
			async applyLocalPlan(plan) {
				timeline.push(`apply:${plan.length}`);
				expect(plan).toHaveLength(1);
				expect(plan[0]).toMatchObject({
					kind: "create",
					serverNodeId: "server-bookmark-1",
					nodeType: "bookmark",
					title: "Pulled Bookmark",
					parentClientExternalId: "root",
					url: "https://pulled.example",
				});
				return { createdMappings: [] };
			},
		};
		const api = {
			async registerClient() {
				throw new Error("register should not be called");
			},
			async startSession() {
				timeline.push("session");
				return {
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					selectedProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					availableProfiles: [],
					libraries: [
						{
							id: "library-1",
							name: "Default",
							kind: "normal",
							projection: "include",
							currentRevisionClock: 2,
						},
					],
					cursors: [],
					serverTime: "2026-05-05T00:00:00Z",
				};
			},
			async feed(request: FeedRequest) {
				timeline.push("feed");
				expect(request.fromClock).toBe(0);
				return {
					browserClientId: "client-1",
					libraryId: "library-1",
					fromClock: 0,
					toClock: 2,
					currentClock: 2,
					serverOps: [
						{
							revId: "rev-2",
							nodeId: "server-bookmark-1",
							opType: "node.create",
							logicalClock: 2,
							payload: {
								node: {
									nodeType: "bookmark",
									parentId: "server-root",
									title: "Pulled Bookmark",
									url: "https://pulled.example",
									sortKey: "a1",
								},
							},
							createdAt: "2026-05-05T00:00:00Z",
						},
					],
					nextCursor: null,
				};
			},
			async preview(request: SyncPreviewRequest) {
				timeline.push("preview");
				expect(request.baseClock).toBe(0);
				expect(request.localMutations).toHaveLength(0);
				return {
					previewId: "preview-pull-1",
					expiresAt: "2026-05-05T00:10:00Z",
					summary: {
						serverToLocal: 1,
						localToServerAccepted: 0,
						conflicts: 0,
					},
					serverOps: [
						{
							revId: "rev-2",
							nodeId: "server-bookmark-1",
							opType: "node.create",
							logicalClock: 2,
							payload: {
								node: {
									nodeType: "bookmark",
									parentId: "server-root",
									title: "Pulled Bookmark",
									url: "https://pulled.example",
									sortKey: "a1",
								},
							},
							createdAt: "2026-05-05T00:00:00Z",
						},
					],
					acceptedLocalMutations: [],
					conflicts: [],
				};
			},
			async apply() {
				timeline.push("apply");
				return {
					applied: true,
					newClock: 2,
					serverOpsToApplyLocally: [
						{
							revId: "rev-2",
							nodeId: "server-bookmark-1",
							opType: "node.create",
							logicalClock: 2,
							payload: {
								node: {
									nodeType: "bookmark",
									parentId: "server-root",
									title: "Pulled Bookmark",
									url: "https://pulled.example",
									sortKey: "a1",
								},
							},
							createdAt: "2026-05-05T00:00:00Z",
						},
					],
					createdMappings: [],
					conflicts: [],
				};
			},
			async ackCursor(request: CursorAckRequest) {
				timeline.push("ack");
				expect(timeline).toContain("apply:1");
				expect(request.appliedClock).toBe(2);
				return {
					cursor: {
						browserClientId: "client-1",
						libraryId: "library-1",
						lastAppliedClock: 2,
						lastAckRevId: "rev-2",
						lastSyncAt: null,
					},
				};
			},
		};

		const result = await runManualSync({
			api,
			adapter,
			storage,
			auth: {
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
			},
			confirmApply: () => true,
		});

		expect(result.status).toBe("synced");
		expect(timeline).toEqual([
			"capabilities",
			"session",
			"feed",
			"loadTree",
			"preview",
			"apply",
			"apply:1",
			"loadTree",
			"ack",
		]);
		expect(
			(await storage.loadState()).cursorsByLibraryId["library-1"]
				?.lastAppliedClock,
		).toBe(2);
	});

	it("stores pending preview and does not apply or ack when server reports stale_base_clock", async () => {
		const calls: string[] = [];
		const storage = createMemorySyncStorage({
			browserClientId: "client-1",
			selectedProfileId: "profile-1",
			cursorsByLibraryId: {
				"library-1": {
					browserClientId: "client-1",
					libraryId: "library-1",
					lastAppliedClock: 0,
					lastAckRevId: null,
					lastSyncAt: null,
				},
			},
		});
		const adapter = new FakeAdapter([
			{
				clientExternalId: "root",
				parentClientExternalId: null,
				nodeType: "folder",
				title: "Bookmarks Bar",
				url: null,
				sortKey: "a",
				children: [
					{
						clientExternalId: "created-local",
						parentClientExternalId: "root",
						nodeType: "bookmark",
						title: "Local Create",
						url: "https://local.example",
						sortKey: "a1",
					},
				],
			},
		]);
		const api = {
			async registerClient() {
				throw new Error("register should not be called");
			},
			async startSession() {
				calls.push("session");
				return {
					browserClient: {
						id: "client-1",
						deviceId: "device-1",
						browserFamily: "chrome",
						browserProfileName: "Default",
						extensionInstanceId: "ext-1",
						capabilities: {},
						lastSeenAt: null,
					},
					selectedProfile: {
						id: "profile-1",
						name: "Default",
						mode: "manual",
						defaultDirection: "bidirectional",
						conflictPolicy: "manual",
						enabled: true,
						rules: [],
					},
					availableProfiles: [],
					libraries: [
						{
							id: "library-1",
							name: "Default",
							kind: "normal",
							projection: "include",
							currentRevisionClock: 2,
						},
					],
					cursors: [],
					serverTime: "2026-05-05T00:00:00Z",
				};
			},
			async feed(request: FeedRequest) {
				calls.push("feed");
				expect(request.fromClock).toBe(0);
				return {
					browserClientId: "client-1",
					libraryId: "library-1",
					fromClock: 0,
					toClock: 2,
					currentClock: 2,
					serverOps: [],
					nextCursor: null,
				};
			},
			async preview(request: SyncPreviewRequest) {
				calls.push("preview");
				expect(request.baseClock).toBe(0);
				expect(request.localMutations).toHaveLength(1);
				return {
					previewId: "preview-stale-1",
					expiresAt: "2026-05-05T00:10:00Z",
					summary: {
						serverToLocal: 2,
						localToServerAccepted: 0,
						conflicts: 1,
					},
					serverOps: [],
					acceptedLocalMutations: [],
					conflicts: [
						{
							conflictType: "stale_base_clock",
							summary:
								"pull newer server revisions before retrying local mutations",
							details: {
								baseClock: 0,
								currentClock: 2,
							},
						},
					],
				};
			},
			async apply() {
				throw new Error("apply should not be called");
			},
			async ackCursor() {
				throw new Error("ack should not be called");
			},
		};

		const result = await runManualSync({
			api,
			adapter,
			storage,
			auth: {
				deviceName: "My Mac",
				deviceType: "desktop",
				platform: "macos",
				browserFamily: "chrome",
				browserProfileName: "Default",
				extensionInstanceId: "ext-1",
			},
			confirmApply: () => true,
		});

		expect(result.status).toBe("needs-user-resolution");
		expect(calls).toEqual(["session", "feed", "preview"]);
		expect(adapter.calls).toEqual(["capabilities", "loadTree"]);
		const savedState = await storage.loadState();
		expect(savedState.pendingPreview?.preview.previewId).toBe(
			"preview-stale-1",
		);
		expect(savedState.pendingRecovery).toBeNull();
		expect(savedState.cursorsByLibraryId["library-1"]?.lastAppliedClock).toBe(
			0,
		);
	});
});

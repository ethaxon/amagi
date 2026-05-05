export interface BrowserBookmarkNodeLike {
	id: string;
	parentId?: string;
	index?: number;
	title?: string;
	url?: string;
	children?: BrowserBookmarkNodeLike[];
}

export interface BrowserBookmarksLike {
	getTree(): Promise<BrowserBookmarkNodeLike[]>;
	create(details: {
		parentId?: string;
		index?: number;
		title?: string;
		url?: string;
	}): Promise<BrowserBookmarkNodeLike>;
	update(
		id: string,
		changes: { title?: string; url?: string },
	): Promise<BrowserBookmarkNodeLike>;
	move(
		id: string,
		destination: { parentId?: string; index?: number },
	): Promise<BrowserBookmarkNodeLike>;
	remove?(id: string): Promise<void>;
	removeTree?(id: string): Promise<void>;
}

export interface BrowserStorageAreaLike {
	get(
		keys?: string | string[] | Record<string, unknown> | null,
	): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
}

export interface BrowserLike {
	bookmarks?: BrowserBookmarksLike;
	storage?: {
		local?: BrowserStorageAreaLike;
	};
	chrome?: {
		runtime?: {
			getManifest?: () => { manifest_version?: number };
		};
	};
	runtime?: {
		getBrowserInfo?: () => Promise<{ name?: string }>;
		getManifest?: () => { manifest_version?: number };
		sendMessage?: (message: unknown) => Promise<unknown>;
		onMessage?: {
			addListener(handler: (message: unknown) => unknown): void;
		};
	};
}

export const BrowserFamily = {
	Chrome: "chrome",
	Firefox: "firefox",
	Safari: "safari",
	Unknown: "unknown",
} as const;

export type BrowserFamily = (typeof BrowserFamily)[keyof typeof BrowserFamily];

export interface WebExtCapabilities {
	canReadBookmarks: boolean;
	canWriteBookmarks: boolean;
	canUseStorage: boolean;
	browserFamily?: BrowserFamily;
	manifestVersion?: number;
}

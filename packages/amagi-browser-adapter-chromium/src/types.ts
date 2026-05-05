export interface ChromiumBookmarkNode {
	id: string;
	parentId?: string;
	index?: number;
	title?: string;
	url?: string;
	children?: ChromiumBookmarkNode[];
}

export interface ChromiumBookmarksApi {
	getTree(): Promise<ChromiumBookmarkNode[]>;
	create(details: {
		parentId?: string;
		index?: number;
		title?: string;
		url?: string;
	}): Promise<ChromiumBookmarkNode>;
	update(
		id: string,
		changes: { title?: string; url?: string },
	): Promise<ChromiumBookmarkNode>;
	move(
		id: string,
		destination: { parentId?: string; index?: number },
	): Promise<ChromiumBookmarkNode>;
	remove?(id: string): Promise<void>;
	removeTree(id: string): Promise<void>;
}

export interface ChromiumStorageArea {
	get(
		keys?: string | string[] | Record<string, unknown> | null,
	): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
}

export interface ChromiumLike {
	bookmarks: ChromiumBookmarksApi;
	storage: {
		local: ChromiumStorageArea;
	};
}

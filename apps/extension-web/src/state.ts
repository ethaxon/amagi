export interface ExtensionConfig {
	apiBaseUrl: string;
	oidcSource: string;
	devBearerToken: string;
}

export interface ExtensionViewState {
	status: string;
	message: string;
	previewSummary: {
		serverToLocal: number;
		localToServerAccepted: number;
		conflicts: number;
	} | null;
}

export const EXTENSION_CONFIG_KEY = "amagi.extension.config";

export const defaultExtensionConfig: ExtensionConfig = {
	apiBaseUrl: "http://localhost:3000",
	oidcSource: "primary",
	devBearerToken: "",
};

export const defaultExtensionViewState: ExtensionViewState = {
	status: "idle",
	message: "manual sync is idle",
	previewSummary: null,
};

export async function loadExtensionConfig(storageArea: {
	get(
		keys?: string | string[] | Record<string, unknown> | null,
	): Promise<Record<string, unknown>>;
}): Promise<ExtensionConfig> {
	const stored = await storageArea.get(EXTENSION_CONFIG_KEY);
	const config = stored[EXTENSION_CONFIG_KEY];
	if (typeof config !== "object" || config === null) {
		return defaultExtensionConfig;
	}
	return {
		...defaultExtensionConfig,
		...(config as Partial<ExtensionConfig>),
	};
}

export async function saveExtensionConfig(
	storageArea: {
		set(items: Record<string, unknown>): Promise<void>;
	},
	config: ExtensionConfig,
): Promise<void> {
	await storageArea.set({ [EXTENSION_CONFIG_KEY]: config });
}

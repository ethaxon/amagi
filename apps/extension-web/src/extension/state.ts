import type { ManualSyncStatus } from "@ethaxon/amagi-sync-client";

export interface ExtensionConfig {
	apiBaseUrl: string;
	oidcSource: string;
	devBearerToken: string;
}

export type ExtensionConfigValidationResult =
	| {
			isValid: true;
			config: ExtensionConfig;
	  }
	| {
			isValid: false;
			message: string;
	  };

export interface ExtensionViewState {
	status: ExtensionViewStateStatus;
	message: string;
	previewSummary: {
		serverToLocal: number;
		localToServerAccepted: number;
		conflicts: number;
	} | null;
}

export const ExtensionViewStateStatus = {
	Idle: "idle",
	Error: "error",
} as const;

export type ExtensionViewStateStatus =
	| (typeof ExtensionViewStateStatus)[keyof typeof ExtensionViewStateStatus]
	| ManualSyncStatus;

export const ExtensionAuthStateStatus = {
	Authenticated: "authenticated",
	Unauthenticated: "unauthenticated",
	Error: "error",
} as const;

export type ExtensionAuthStateStatus =
	(typeof ExtensionAuthStateStatus)[keyof typeof ExtensionAuthStateStatus];

export interface ExtensionAuthState {
	status: ExtensionAuthStateStatus;
	message: string;
	displayName: string | null;
	subject: string | null;
	usesDevBearerFallback: boolean;
}

export const EXTENSION_CONFIG_KEY = "amagi.extension.config";

export const defaultExtensionConfig: ExtensionConfig = {
	apiBaseUrl: "http://127.0.0.1:7800",
	oidcSource: "default",
	devBearerToken: "",
};

export const defaultExtensionViewState: ExtensionViewState = {
	status: ExtensionViewStateStatus.Idle,
	message: "manual sync is idle",
	previewSummary: null,
};

export function validateExtensionConfig(
	config: ExtensionConfig,
): ExtensionConfigValidationResult {
	const normalizedConfig = {
		apiBaseUrl: config.apiBaseUrl.trim(),
		oidcSource: config.oidcSource.trim(),
		devBearerToken: config.devBearerToken.trim(),
	};

	if (!normalizedConfig.apiBaseUrl) {
		return {
			isValid: false,
			message: "API Base URL is required.",
		};
	}

	let parsedUrl: URL;
	try {
		parsedUrl = new URL(normalizedConfig.apiBaseUrl);
	} catch {
		return {
			isValid: false,
			message: "API Base URL must be a valid absolute URL.",
		};
	}

	if (parsedUrl.protocol === "https:") {
		// Allowed for future self-hosted deployments.
	} else if (
		parsedUrl.protocol === "http:" &&
		(parsedUrl.hostname === "localhost" || parsedUrl.hostname === "127.0.0.1")
	) {
		// Allowed for local development smoke paths.
	} else {
		return {
			isValid: false,
			message:
				"API Base URL must use https://, http://localhost, or http://127.0.0.1.",
		};
	}

	if (!normalizedConfig.oidcSource) {
		return {
			isValid: false,
			message: "OIDC Source is required.",
		};
	}

	return {
		isValid: true,
		config: normalizedConfig,
	};
}

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

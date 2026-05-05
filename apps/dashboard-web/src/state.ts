export interface DashboardConnectionConfig {
	apiBaseUrl: string;
	oidcSource: string;
	devBearerToken: string;
}

export type DashboardConnectionValidationResult =
	| {
			isValid: true;
			config: DashboardConnectionConfig;
	  }
	| {
			isValid: false;
			message: string;
	  };

export interface StorageLike {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export const DASHBOARD_CONNECTION_STORAGE_KEY =
	"amagi.dashboard.dev.connection";

export const defaultDashboardConnectionConfig: DashboardConnectionConfig = {
	apiBaseUrl: "http://127.0.0.1:7800",
	oidcSource: "default",
	devBearerToken: "",
};

export function validateDashboardConnectionConfig(
	config: DashboardConnectionConfig,
): DashboardConnectionValidationResult {
	const normalizedConfig = {
		apiBaseUrl: config.apiBaseUrl.trim(),
		oidcSource: config.oidcSource.trim(),
		devBearerToken: config.devBearerToken.trim(),
	};

	if (!normalizedConfig.apiBaseUrl) {
		return { isValid: false, message: "API Base URL is required." };
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

	if (
		parsedUrl.protocol !== "https:" &&
		!(
			parsedUrl.protocol === "http:" &&
			(parsedUrl.hostname === "localhost" || parsedUrl.hostname === "127.0.0.1")
		)
	) {
		return {
			isValid: false,
			message:
				"API Base URL must use https://, http://localhost, or http://127.0.0.1.",
		};
	}

	if (!normalizedConfig.oidcSource) {
		return { isValid: false, message: "OIDC Source is required." };
	}

	return { isValid: true, config: normalizedConfig };
}

export function loadDashboardConnectionConfig(
	storage: StorageLike | undefined,
): DashboardConnectionConfig {
	if (!storage) {
		return defaultDashboardConnectionConfig;
	}
	const rawValue = storage.getItem(DASHBOARD_CONNECTION_STORAGE_KEY);
	if (!rawValue) {
		return defaultDashboardConnectionConfig;
	}
	try {
		const parsed = JSON.parse(rawValue) as Partial<DashboardConnectionConfig>;
		return {
			...defaultDashboardConnectionConfig,
			...parsed,
		};
	} catch {
		return defaultDashboardConnectionConfig;
	}
}

export function saveDashboardConnectionConfig(
	storage: StorageLike | undefined,
	config: DashboardConnectionConfig,
): void {
	if (!storage) {
		return;
	}
	storage.setItem(DASHBOARD_CONNECTION_STORAGE_KEY, JSON.stringify(config));
}

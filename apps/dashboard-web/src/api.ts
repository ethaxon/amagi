import type {
	ConflictPolicy,
	DefaultDirection,
	RuleAction,
	RuleMatcherType,
	SyncProfileMode,
} from "./constants";
import {
	type DashboardConnectionConfig,
	validateDashboardConnectionConfig,
} from "./state";

export interface DashboardApiErrorPayload {
	code: string;
	message: string;
	source: string | null;
}

export class DashboardApiError extends Error {
	public readonly code: string;
	public readonly source: string | null;

	public constructor(payload: DashboardApiErrorPayload) {
		super(payload.message);
		this.name = "DashboardApiError";
		this.code = payload.code;
		this.source = payload.source;
	}
}

export interface SyncProfileRuleView {
	id: string;
	ruleOrder: number;
	action: RuleAction;
	matcherType: RuleMatcherType;
	matcherValue: string;
	options: Record<string, unknown>;
}

export interface SyncProfileTargetView {
	id: string;
	platform: string | null;
	deviceType: string | null;
	deviceId: string | null;
	browserFamily: string | null;
	browserClientId: string | null;
}

export interface SyncProfileDetailView {
	id: string;
	name: string;
	mode: SyncProfileMode;
	defaultDirection: DefaultDirection;
	conflictPolicy: ConflictPolicy;
	enabled: boolean;
	rules: SyncProfileRuleView[];
	targets: SyncProfileTargetView[];
}

export interface CreateSyncProfileInput {
	name: string;
	mode: SyncProfileMode;
	defaultDirection: DefaultDirection;
	conflictPolicy: ConflictPolicy;
}

export interface UpdateSyncProfileInput {
	name?: string;
	enabled?: boolean;
	defaultDirection?: DefaultDirection;
	conflictPolicy?: ConflictPolicy;
}

export interface CreateSyncProfileTargetInput {
	platform?: string;
	deviceType?: string;
	deviceId?: string;
	browserFamily?: string;
	browserClientId?: string;
}

export interface CreateSyncProfileRuleInput {
	ruleOrder: number;
	action: RuleAction;
	matcherType: RuleMatcherType;
	matcherValue: string;
	options?: Record<string, unknown>;
}

export interface UpdateSyncProfileRuleInput {
	ruleOrder?: number;
	action?: RuleAction;
	matcherType?: RuleMatcherType;
	matcherValue?: string;
	options?: Record<string, unknown>;
}

export interface DashboardApiClientOptions {
	connection: DashboardConnectionConfig;
	authorizationHeader?: string | null;
	fetchImpl?: typeof fetch;
}

export function createDashboardApiClient(options: DashboardApiClientOptions) {
	const validatedConnection = validateDashboardConnectionConfig(
		options.connection,
	);
	if (!validatedConnection.isValid) {
		throw new Error(validatedConnection.message);
	}

	const connection = validatedConnection.config;
	const authorizationHeader = options.authorizationHeader ?? null;
	const fetchImpl = options.fetchImpl ?? fetch;

	return {
		listSyncProfiles() {
			return request<SyncProfileDetailView[]>(
				"/api/v1/dashboard/sync-profiles",
				{
					method: "GET",
				},
			);
		},
		createSyncProfile(input: CreateSyncProfileInput) {
			return request<SyncProfileDetailView>("/api/v1/dashboard/sync-profiles", {
				method: "POST",
				body: JSON.stringify(input),
			});
		},
		updateSyncProfile(options: {
			profileId: string;
			input: UpdateSyncProfileInput;
		}) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}`,
				{
					method: "PATCH",
					body: JSON.stringify(options.input),
				},
			);
		},
		createSyncProfileTarget(options: {
			profileId: string;
			input: CreateSyncProfileTargetInput;
		}) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}/targets`,
				{
					method: "POST",
					body: JSON.stringify(options.input),
				},
			);
		},
		deleteSyncProfileTarget(options: { profileId: string; targetId: string }) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}/targets/${options.targetId}`,
				{ method: "DELETE" },
			);
		},
		createSyncProfileRule(options: {
			profileId: string;
			input: CreateSyncProfileRuleInput;
		}) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}/rules`,
				{
					method: "POST",
					body: JSON.stringify({
						...options.input,
						options: options.input.options ?? {},
					}),
				},
			);
		},
		updateSyncProfileRule(options: {
			profileId: string;
			ruleId: string;
			input: UpdateSyncProfileRuleInput;
		}) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}/rules/${options.ruleId}`,
				{
					method: "PATCH",
					body: JSON.stringify(options.input),
				},
			);
		},
		deleteSyncProfileRule(options: { profileId: string; ruleId: string }) {
			return request<SyncProfileDetailView>(
				`/api/v1/dashboard/sync-profiles/${options.profileId}/rules/${options.ruleId}`,
				{ method: "DELETE" },
			);
		},
	};

	async function request<T>(path: string, init: RequestInit): Promise<T> {
		const response = await fetchImpl(
			new URL(path, normalizeBaseUrl(connection.apiBaseUrl)),
			{
				...init,
				headers: {
					accept: "application/json",
					...(authorizationHeader
						? { authorization: authorizationHeader }
						: connection.devBearerToken
							? { authorization: `Bearer ${connection.devBearerToken}` }
							: {}),
					"x-amagi-oidc-source": connection.oidcSource,
					...(init.method === "GET"
						? {}
						: { "content-type": "application/json" }),
					...(init.headers ?? {}),
				},
			},
		);

		const text = await response.text();
		const parsed = text ? tryParseJson(text) : null;
		if (!response.ok) {
			const payload = isDashboardApiErrorPayload(parsed)
				? parsed
				: {
						code: "http_error",
						message: text || `${response.status} ${response.statusText}`,
						source: null,
					};
			throw new DashboardApiError(payload);
		}

		if (parsed === null) {
			throw new Error(`dashboard API returned an empty body for ${path}`);
		}

		return parsed as T;
	}
}

function normalizeBaseUrl(value: string): string {
	return value.endsWith("/") ? value : `${value}/`;
}

function tryParseJson(value: string): unknown | null {
	try {
		return JSON.parse(value) as unknown;
	} catch {
		return null;
	}
}

function isDashboardApiErrorPayload(
	value: unknown,
): value is DashboardApiErrorPayload {
	return (
		typeof value === "object" &&
		value !== null &&
		"code" in value &&
		typeof value.code === "string" &&
		"message" in value &&
		typeof value.message === "string" &&
		"source" in value &&
		(typeof value.source === "string" || value.source === null)
	);
}

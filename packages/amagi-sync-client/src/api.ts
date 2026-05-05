import { AmagiApiError, redactSensitiveText, SyncClientError } from "./errors";
import type {
	AmagiSyncApi,
	CursorAckRequest,
	CursorAckResponse,
	FeedRequest,
	FeedResponse,
	RegisterClientRequest,
	RegisterClientResponse,
	SyncApplyRequest,
	SyncApplyResponse,
	SyncPreviewRequest,
	SyncPreviewResponse,
	SyncSessionStartRequest,
	SyncSessionStartResponse,
} from "./types";

export interface AmagiSyncApiClientOptions {
	baseUrl: string;
	bearerToken: string;
	oidcSource?: string | null;
	fetchImpl?: typeof fetch;
}

interface ApiErrorBody {
	code?: unknown;
	message?: unknown;
	source?: unknown;
}

export class AmagiSyncApiClient implements AmagiSyncApi {
	private readonly baseUrl: string;
	private readonly bearerToken: string;
	private readonly oidcSource: string | null;
	private readonly fetchImpl: typeof fetch;

	public constructor(options: AmagiSyncApiClientOptions) {
		this.baseUrl = normalizeBaseUrl(options.baseUrl);
		this.bearerToken = options.bearerToken.trim();
		this.oidcSource = options.oidcSource?.trim() || null;
		this.fetchImpl = options.fetchImpl ?? fetch;
		if (!this.bearerToken) {
			throw new SyncClientError(
				"bearer token is required for sync API requests",
			);
		}
	}

	public registerClient(
		request: RegisterClientRequest,
	): Promise<RegisterClientResponse> {
		return this.request("/api/v1/sync/clients/register", {
			method: "POST",
			body: JSON.stringify(request),
		});
	}

	public startSession(
		request: SyncSessionStartRequest,
	): Promise<SyncSessionStartResponse> {
		return this.request("/api/v1/sync/session/start", {
			method: "POST",
			body: JSON.stringify(request),
		});
	}

	public feed(request: FeedRequest): Promise<FeedResponse> {
		const query = new URLSearchParams();
		query.set("browserClientId", request.browserClientId);
		query.set("libraryId", request.libraryId);
		query.set("fromClock", String(request.fromClock));
		if (request.profileId) {
			query.set("profileId", request.profileId);
		}
		if (typeof request.limit === "number") {
			query.set("limit", String(request.limit));
		}

		return this.request(`/api/v1/sync/feed?${query.toString()}`, {
			method: "GET",
		});
	}

	public preview(request: SyncPreviewRequest): Promise<SyncPreviewResponse> {
		return this.request("/api/v1/sync/preview", {
			method: "POST",
			body: JSON.stringify(request),
		});
	}

	public apply(request: SyncApplyRequest): Promise<SyncApplyResponse> {
		return this.request("/api/v1/sync/apply", {
			method: "POST",
			body: JSON.stringify(request),
		});
	}

	public ackCursor(request: CursorAckRequest): Promise<CursorAckResponse> {
		return this.request("/api/v1/sync/cursors/ack", {
			method: "POST",
			body: JSON.stringify(request),
		});
	}

	private async request<T>(path: string, init: RequestInit): Promise<T> {
		const response = await this.fetchImpl(new URL(path, this.baseUrl), {
			...init,
			headers: {
				accept: "application/json",
				authorization: `Bearer ${this.bearerToken}`,
				...(init.method === "GET"
					? {}
					: { "content-type": "application/json" }),
				...(this.oidcSource ? { "x-amagi-oidc-source": this.oidcSource } : {}),
				...(init.headers ?? {}),
			},
		});

		const text = await response.text();
		const parsed = text ? tryParseJson(text) : null;
		if (!response.ok) {
			const errorBody = isApiErrorBody(parsed) ? parsed : null;
			throw new AmagiApiError({
				code:
					typeof errorBody?.code === "string" ? errorBody.code : "http_error",
				message:
					typeof errorBody?.message === "string"
						? errorBody.message
						: redactSensitiveText(
								text || `${response.status} ${response.statusText}`,
							),
				status: response.status,
				source: typeof errorBody?.source === "string" ? errorBody.source : null,
			});
		}

		if (parsed === null) {
			throw new SyncClientError(`sync API returned an empty body for ${path}`);
		}

		return parsed as T;
	}
}

function normalizeBaseUrl(baseUrl: string): string {
	const trimmed = baseUrl.trim();
	if (!trimmed) {
		throw new SyncClientError("baseUrl is required for sync API client");
	}
	return trimmed.endsWith("/") ? trimmed : `${trimmed}/`;
}

function tryParseJson(value: string): unknown | null {
	try {
		return JSON.parse(value) as unknown;
	} catch {
		return null;
	}
}

function isApiErrorBody(value: unknown): value is ApiErrorBody {
	return typeof value === "object" && value !== null;
}

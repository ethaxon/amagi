const BEARER_PATTERN = /Bearer\s+[A-Za-z0-9._~+/=-]+/giu;

export class AmagiApiError extends Error {
	public readonly code: string;
	public readonly status: number;
	public readonly source: string | null;

	public constructor(options: {
		code: string;
		message: string;
		status: number;
		source?: string | null;
	}) {
		super(redactSensitiveText(options.message));
		this.name = "AmagiApiError";
		this.code = options.code;
		this.status = options.status;
		this.source = options.source ?? null;
	}
}

export class SyncClientError extends Error {
	public constructor(message: string) {
		super(redactSensitiveText(message));
		this.name = "SyncClientError";
	}
}

export function redactSensitiveText(value: string): string {
	return value.replace(BEARER_PATTERN, "Bearer [redacted]");
}

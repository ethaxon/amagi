import {
	type AmagiAuthClient,
	AmagiAuthHost,
	createAmagiAuthClient,
} from "@ethaxon/amagi-auth-client";
import { startTransition, useEffect, useState } from "react";

import {
	createDashboardApiClient,
	type DashboardApiErrorPayload,
	type SyncProfileDetailView,
} from "./api";
import {
	CONFLICT_POLICIES,
	type ConflictPolicy,
	DEFAULT_DIRECTIONS,
	type DefaultDirection,
	RULE_ACTIONS,
	RULE_MATCHER_TYPES,
	type RuleAction,
	type RuleMatcherType,
} from "./constants";
import {
	type DashboardConnectionConfig,
	defaultDashboardConnectionConfig,
	loadDashboardConnectionConfig,
	saveDashboardConnectionConfig,
	validateDashboardConnectionConfig,
} from "./state";

interface RuleDraft {
	ruleOrder: number;
	action: RuleAction;
	matcherType: RuleMatcherType;
	matcherValue: string;
}

interface TargetDraft {
	platform: string;
	deviceType: string;
	deviceId: string;
	browserFamily: string;
	browserClientId: string;
}

const defaultTargetDraft: TargetDraft = {
	platform: "",
	deviceType: "",
	deviceId: "",
	browserFamily: "",
	browserClientId: "",
};

const DASHBOARD_POST_AUTH_REDIRECT_STORAGE_KEY =
	"amagi.dashboard.post-auth-redirect";

const AuthPanelStatus = {
	Loading: "loading",
	Authenticated: "authenticated",
	Unauthenticated: "unauthenticated",
	Error: "error",
} as const;

type AuthPanelStatus = (typeof AuthPanelStatus)[keyof typeof AuthPanelStatus];

export function App() {
	const [authClient, setAuthClient] = useState<AmagiAuthClient>(() =>
		createDashboardAuthClient(defaultDashboardConnectionConfig),
	);
	const [authStatus, setAuthStatus] = useState<AuthPanelStatus>(
		AuthPanelStatus.Loading,
	);
	const [authMessage, setAuthMessage] = useState(
		"Checking local token-set auth state...",
	);
	const [authAuthorizationHeader, setAuthAuthorizationHeader] = useState<
		string | null
	>(null);
	const [authDisplayName, setAuthDisplayName] = useState<string | null>(null);
	const [authSubject, setAuthSubject] = useState<string | null>(null);
	const [currentPathname, setCurrentPathname] = useState(
		() => globalThis.location.pathname,
	);
	const [connectionConfig, setConnectionConfig] =
		useState<DashboardConnectionConfig>(defaultDashboardConnectionConfig);
	const [profiles, setProfiles] = useState<SyncProfileDetailView[]>([]);
	const [selectedProfileId, setSelectedProfileId] = useState<string | null>(
		null,
	);
	const [connectionMessage, setConnectionMessage] = useState(
		"Load a dev sync profile list to start editing policy.",
	);
	const [lastError, setLastError] = useState<DashboardApiErrorPayload | null>(
		null,
	);
	const [isLoading, setIsLoading] = useState(false);
	const [createProfileName, setCreateProfileName] =
		useState("Desktop Browsers");
	const [createProfileDirection, setCreateProfileDirection] =
		useState<DefaultDirection>("bidirectional");
	const [profileDraftName, setProfileDraftName] = useState("");
	const [profileDraftEnabled, setProfileDraftEnabled] = useState(true);
	const [profileDraftDirection, setProfileDraftDirection] =
		useState<DefaultDirection>("bidirectional");
	const [profileDraftConflictPolicy, setProfileDraftConflictPolicy] =
		useState<ConflictPolicy>("manual");
	const [targetDraft, setTargetDraft] =
		useState<TargetDraft>(defaultTargetDraft);
	const [newRuleDraft, setNewRuleDraft] = useState<RuleDraft>({
		ruleOrder: 10,
		action: "include",
		matcherType: "tag",
		matcherValue: "",
	});
	const [ruleDrafts, setRuleDrafts] = useState<Record<string, RuleDraft>>({});

	const selectedProfile =
		profiles.find((profile) => profile.id === selectedProfileId) ?? null;
	const hasDevBearerFallback =
		connectionConfig.devBearerToken.trim().length > 0;
	const canCallDashboardApi =
		authAuthorizationHeader !== null || hasDevBearerFallback;
	const isCallbackPath = currentPathname === authClient.paths.callbackPath;

	useEffect(() => {
		setConnectionConfig(loadDashboardConnectionConfig(globalThis.localStorage));
	}, []);

	useEffect(() => {
		setAuthClient(
			createDashboardAuthClient({
				apiBaseUrl: connectionConfig.apiBaseUrl,
				oidcSource: connectionConfig.oidcSource,
			}),
		);
	}, [connectionConfig.apiBaseUrl, connectionConfig.oidcSource]);

	useEffect(() => {
		let cancelled = false;

		setAuthStatus("loading");
		setAuthMessage(
			isCallbackPath
				? "Completing backend-oidc callback..."
				: "Checking local token-set auth state...",
		);

		void authClient
			.ensureReady()
			.then(async (snapshot) => {
				if (cancelled) {
					return;
				}

				const authorizationHeader = authClient.authorizationHeader();
				setAuthAuthorizationHeader(authorizationHeader);

				if (!snapshot || !authorizationHeader) {
					setAuthStatus("unauthenticated");
					setAuthDisplayName(null);
					setAuthSubject(null);
					setAuthMessage(
						hasDevBearerFallback
							? "Not signed in. Advanced dev bearer fallback is configured for debugging."
							: "Sign in with Dex amagi/amagi to call Dashboard APIs.",
					);
					return;
				}

				await authClient.loadBoundUserInfo();

				if (cancelled) {
					return;
				}

				setAuthStatus("authenticated");
				setAuthDisplayName(readPrincipalDisplayName(snapshot));
				setAuthSubject(readPrincipalSubject(snapshot));
				setAuthMessage(
					"Authenticated via backend-oidc token-set. Dashboard API calls now use the SDK bearer principal.",
				);

				if (isCallbackPath) {
					const nextPath =
						consumeDashboardPostAuthRedirect(globalThis.sessionStorage) ?? "/";
					globalThis.history.replaceState({}, "", nextPath);
					setCurrentPathname(
						new URL(nextPath, globalThis.location.origin).pathname,
					);
				}
			})
			.catch((error: unknown) => {
				if (cancelled) {
					return;
				}
				const nextError = toDashboardErrorPayload(error);
				setAuthStatus("error");
				setAuthAuthorizationHeader(null);
				setAuthDisplayName(null);
				setAuthSubject(null);
				setAuthMessage(`${nextError.code}: ${nextError.message}`);
			});

		return () => {
			cancelled = true;
		};
	}, [authClient, hasDevBearerFallback, isCallbackPath]);

	useEffect(() => {
		if (!selectedProfile) {
			setProfileDraftName("");
			setRuleDrafts({});
			return;
		}
		setProfileDraftName(selectedProfile.name);
		setProfileDraftEnabled(selectedProfile.enabled);
		setProfileDraftDirection(selectedProfile.defaultDirection);
		setProfileDraftConflictPolicy(selectedProfile.conflictPolicy);
		setRuleDrafts(
			Object.fromEntries(
				selectedProfile.rules.map((rule) => [
					rule.id,
					{
						ruleOrder: rule.ruleOrder,
						action: rule.action,
						matcherType: rule.matcherType,
						matcherValue: rule.matcherValue,
					},
				]),
			),
		);
	}, [selectedProfile]);

	return (
		<div className="dashboard-shell">
			<aside className="dashboard-sidebar">
				<section className="panel">
					<p className="eyebrow">Auth Panel</p>
					<h1>Sync Profiles</h1>
					<p className="panel-copy">
						Use the local SecurityDept token-set flow with Dex `amagi/amagi`.
						The bearer textarea remains available only as an advanced debugging
						fallback.
					</p>
					<div className="auth-status-grid">
						<div className="auth-status-row">
							<span className={`auth-chip ${authStatus}`}>{authStatus}</span>
							{authDisplayName ? <strong>{authDisplayName}</strong> : null}
						</div>
						<div className="auth-identity">
							<span>{authMessage}</span>
							{authSubject ? <span className="mono">{authSubject}</span> : null}
						</div>
					</div>
					<div className="field-stack">
						<label>
							<span>API Base URL</span>
							<input
								value={connectionConfig.apiBaseUrl}
								onChange={(event) =>
									setConnectionConfig((current) => ({
										...current,
										apiBaseUrl: event.currentTarget.value,
									}))
								}
							/>
						</label>
						<label>
							<span>OIDC Source</span>
							<input
								value={connectionConfig.oidcSource}
								onChange={(event) =>
									setConnectionConfig((current) => ({
										...current,
										oidcSource: event.currentTarget.value,
									}))
								}
							/>
						</label>
					</div>
					<div className="button-row">
						<button
							disabled={isLoading || authStatus === "loading"}
							onClick={() => handleLogin()}
							type="button"
						>
							Login
						</button>
						<button
							disabled={
								isLoading ||
								(authStatus !== "authenticated" && !canCallDashboardApi)
							}
							onClick={() => void handleLogout()}
							type="button"
						>
							Clear Auth
						</button>
						<button
							disabled={isLoading || !canCallDashboardApi}
							onClick={() => void handleLoadProfiles()}
							type="button"
						>
							{isLoading ? "Loading..." : "Load Profiles"}
						</button>
					</div>
					<p className="status-line">{connectionMessage}</p>
					<details className="advanced-fallback">
						<summary>Advanced Dev Fallback</summary>
						<p>
							Only use this textarea to bypass the frontend SDK while debugging.
							The default happy path should not require manual token copy.
						</p>
						<div className="field-stack compact">
							<label>
								<span>Dev Bearer Token</span>
								<textarea
									rows={3}
									value={connectionConfig.devBearerToken}
									onChange={(event) =>
										setConnectionConfig((current) => ({
											...current,
											devBearerToken: event.currentTarget.value,
										}))
									}
								/>
							</label>
						</div>
					</details>
				</section>

				<section className="panel">
					<div className="panel-header-inline">
						<p className="eyebrow">Profiles</p>
						<span>{profiles.length}</span>
					</div>
					<div className="profile-list">
						{profiles.map((profile) => (
							<button
								className={
									profile.id === selectedProfileId
										? "profile-list-item selected"
										: "profile-list-item"
								}
								disabled={isLoading}
								key={profile.id}
								onClick={() => setSelectedProfileId(profile.id)}
								type="button"
							>
								<strong>{profile.name}</strong>
								<span>{profile.mode}</span>
								<span>
									{profile.enabled ? "enabled" : "disabled"} ·{" "}
									{profile.defaultDirection}
								</span>
							</button>
						))}
					</div>
					<div className="field-stack compact">
						<label>
							<span>New Profile Name</span>
							<input
								value={createProfileName}
								onChange={(event) =>
									setCreateProfileName(event.currentTarget.value)
								}
							/>
						</label>
						<label>
							<span>Default Direction</span>
							<select
								value={createProfileDirection}
								onChange={(event) =>
									setCreateProfileDirection(
										event.currentTarget.value as DefaultDirection,
									)
								}
							>
								{DEFAULT_DIRECTIONS.map((direction) => (
									<option key={direction} value={direction}>
										{direction}
									</option>
								))}
							</select>
						</label>
						<button
							disabled={isLoading || !canCallDashboardApi}
							onClick={() => void handleCreateProfile()}
							type="button"
						>
							Create Manual Profile
						</button>
					</div>
				</section>
			</aside>

			<main className="dashboard-main">
				<section className="panel hero-panel">
					<div>
						<p className="eyebrow">Policy Source</p>
						<h2>
							{selectedProfile
								? selectedProfile.name
								: "Select a profile to inspect rules"}
						</h2>
					</div>
					<div className="hero-meta">
						<span>Targets: {selectedProfile?.targets.length ?? 0}</span>
						<span>Rules: {selectedProfile?.rules.length ?? 0}</span>
					</div>
				</section>

				{lastError ? (
					<section className="panel error-panel">
						<p className="eyebrow">API Error</p>
						<strong>{lastError.code}</strong>
						<p>{lastError.message}</p>
						{lastError.source ? <code>{lastError.source}</code> : null}
					</section>
				) : null}

				{selectedProfile ? (
					<>
						<section className="panel">
							<div className="panel-header-inline">
								<p className="eyebrow">Profile Detail</p>
								<span className="mono">{selectedProfile.id}</span>
							</div>
							<div className="profile-editor-grid">
								<label>
									<span>Name</span>
									<input
										value={profileDraftName}
										onChange={(event) =>
											setProfileDraftName(event.currentTarget.value)
										}
									/>
								</label>
								<label>
									<span>Default Direction</span>
									<select
										value={profileDraftDirection}
										onChange={(event) =>
											setProfileDraftDirection(
												event.currentTarget.value as DefaultDirection,
											)
										}
									>
										{DEFAULT_DIRECTIONS.map((direction) => (
											<option key={direction} value={direction}>
												{direction}
											</option>
										))}
									</select>
								</label>
								<label>
									<span>Conflict Policy</span>
									<select
										value={profileDraftConflictPolicy}
										onChange={(event) =>
											setProfileDraftConflictPolicy(
												event.currentTarget.value as ConflictPolicy,
											)
										}
									>
										{CONFLICT_POLICIES.map((policy) => (
											<option key={policy} value={policy}>
												{policy}
											</option>
										))}
									</select>
								</label>
								<label className="toggle-field">
									<input
										checked={profileDraftEnabled}
										onChange={(event) =>
											setProfileDraftEnabled(event.currentTarget.checked)
										}
										type="checkbox"
									/>
									<span>Enabled</span>
								</label>
							</div>
							<div className="button-row">
								<button
									disabled={isLoading || !canCallDashboardApi}
									onClick={() => void handleSaveProfile()}
									type="button"
								>
									Save Profile
								</button>
							</div>
						</section>

						<section className="panel">
							<div className="panel-header-inline">
								<p className="eyebrow">Targets</p>
								<span>{selectedProfile.targets.length}</span>
							</div>
							<table className="data-table">
								<thead>
									<tr>
										<th>Platform</th>
										<th>Device Type</th>
										<th>Device ID</th>
										<th>Browser</th>
										<th>Browser Client</th>
										<th />
									</tr>
								</thead>
								<tbody>
									{selectedProfile.targets.map((target) => (
										<tr key={target.id}>
											<td>{target.platform ?? "-"}</td>
											<td>{target.deviceType ?? "-"}</td>
											<td className="mono">{target.deviceId ?? "-"}</td>
											<td>{target.browserFamily ?? "-"}</td>
											<td className="mono">{target.browserClientId ?? "-"}</td>
											<td>
												<button
													disabled={isLoading || !canCallDashboardApi}
													onClick={() => void handleDeleteTarget(target.id)}
													type="button"
												>
													Delete
												</button>
											</td>
										</tr>
									))}
								</tbody>
							</table>
							<div className="inline-grid six-columns">
								<input
									placeholder="platform"
									value={targetDraft.platform}
									onChange={(event) =>
										setTargetDraft((current) => ({
											...current,
											platform: event.currentTarget.value,
										}))
									}
								/>
								<input
									placeholder="deviceType"
									value={targetDraft.deviceType}
									onChange={(event) =>
										setTargetDraft((current) => ({
											...current,
											deviceType: event.currentTarget.value,
										}))
									}
								/>
								<input
									placeholder="deviceId"
									value={targetDraft.deviceId}
									onChange={(event) =>
										setTargetDraft((current) => ({
											...current,
											deviceId: event.currentTarget.value,
										}))
									}
								/>
								<input
									placeholder="browserFamily"
									value={targetDraft.browserFamily}
									onChange={(event) =>
										setTargetDraft((current) => ({
											...current,
											browserFamily: event.currentTarget.value,
										}))
									}
								/>
								<input
									placeholder="browserClientId"
									value={targetDraft.browserClientId}
									onChange={(event) =>
										setTargetDraft((current) => ({
											...current,
											browserClientId: event.currentTarget.value,
										}))
									}
								/>
								<button
									disabled={isLoading || !canCallDashboardApi}
									onClick={() => void handleCreateTarget()}
									type="button"
								>
									Add Target
								</button>
							</div>
						</section>

						<section className="panel">
							<div className="panel-header-inline">
								<p className="eyebrow">Rules</p>
								<span>{selectedProfile.rules.length}</span>
							</div>
							<table className="data-table">
								<thead>
									<tr>
										<th>Order</th>
										<th>Action</th>
										<th>Matcher</th>
										<th>Value</th>
										<th />
									</tr>
								</thead>
								<tbody>
									{selectedProfile.rules.map((rule) => {
										const draft = ruleDrafts[rule.id] ?? {
											ruleOrder: rule.ruleOrder,
											action: rule.action,
											matcherType: rule.matcherType,
											matcherValue: rule.matcherValue,
										};
										return (
											<tr key={rule.id}>
												<td>
													<input
														type="number"
														value={draft.ruleOrder}
														onChange={(event) =>
															setRuleDrafts((current) => ({
																...current,
																[rule.id]: {
																	...draft,
																	ruleOrder: Number(event.currentTarget.value),
																},
															}))
														}
													/>
												</td>
												<td>
													<select
														value={draft.action}
														onChange={(event) =>
															setRuleDrafts((current) => ({
																...current,
																[rule.id]: {
																	...draft,
																	action: event.currentTarget
																		.value as RuleAction,
																},
															}))
														}
													>
														{RULE_ACTIONS.map((action) => (
															<option key={action} value={action}>
																{action}
															</option>
														))}
													</select>
												</td>
												<td>
													<select
														value={draft.matcherType}
														onChange={(event) =>
															setRuleDrafts((current) => ({
																...current,
																[rule.id]: {
																	...draft,
																	matcherType: event.currentTarget
																		.value as RuleMatcherType,
																},
															}))
														}
													>
														{RULE_MATCHER_TYPES.map((matcherType) => (
															<option key={matcherType} value={matcherType}>
																{matcherType}
															</option>
														))}
													</select>
												</td>
												<td>
													<input
														value={draft.matcherValue}
														onChange={(event) =>
															setRuleDrafts((current) => ({
																...current,
																[rule.id]: {
																	...draft,
																	matcherValue: event.currentTarget.value,
																},
															}))
														}
													/>
												</td>
												<td className="button-cluster">
													<button
														disabled={isLoading || !canCallDashboardApi}
														onClick={() => void handleSaveRule(rule.id)}
														type="button"
													>
														Save
													</button>
													<button
														disabled={isLoading || !canCallDashboardApi}
														onClick={() => void handleDeleteRule(rule.id)}
														type="button"
													>
														Delete
													</button>
												</td>
											</tr>
										);
									})}
								</tbody>
							</table>
							<div className="inline-grid five-columns">
								<input
									type="number"
									value={newRuleDraft.ruleOrder}
									onChange={(event) =>
										setNewRuleDraft((current) => ({
											...current,
											ruleOrder: Number(event.currentTarget.value),
										}))
									}
								/>
								<select
									value={newRuleDraft.action}
									onChange={(event) =>
										setNewRuleDraft((current) => ({
											...current,
											action: event.currentTarget.value as RuleAction,
										}))
									}
								>
									{RULE_ACTIONS.map((action) => (
										<option key={action} value={action}>
											{action}
										</option>
									))}
								</select>
								<select
									value={newRuleDraft.matcherType}
									onChange={(event) =>
										setNewRuleDraft((current) => ({
											...current,
											matcherType: event.currentTarget.value as RuleMatcherType,
										}))
									}
								>
									{RULE_MATCHER_TYPES.map((matcherType) => (
										<option key={matcherType} value={matcherType}>
											{matcherType}
										</option>
									))}
								</select>
								<input
									placeholder="matcherValue"
									value={newRuleDraft.matcherValue}
									onChange={(event) =>
										setNewRuleDraft((current) => ({
											...current,
											matcherValue: event.currentTarget.value,
										}))
									}
								/>
								<button
									disabled={isLoading || !canCallDashboardApi}
									onClick={() => void handleCreateRule()}
									type="button"
								>
									Add Rule
								</button>
							</div>
						</section>
					</>
				) : (
					<section className="panel empty-panel">
						<p className="eyebrow">No Selection</p>
						<p>
							Load profiles, then select one policy row to inspect targets and
							rules.
						</p>
					</section>
				)}
			</main>
		</div>
	);

	async function handleLoadProfiles() {
		const validation = validateDashboardConnectionConfig(connectionConfig);
		if (!validation.isValid) {
			setConnectionMessage(validation.message);
			setLastError({
				code: "invalid_connection",
				message: validation.message,
				source: null,
			});
			return;
		}

		saveDashboardConnectionConfig(globalThis.localStorage, validation.config);
		if (!canCallDashboardApi) {
			setConnectionMessage(
				"Login is required before loading sync profiles. Use the advanced fallback only for SDK debugging.",
			);
			setLastError({
				code: "unauthenticated",
				message:
					"dashboard API calls require an authenticated token-set session or the advanced dev fallback token.",
				source: null,
			});
			return;
		}
		setIsLoading(true);
		setConnectionMessage("Loading sync profile list...");
		setLastError(null);
		try {
			const client = createDashboardApiClient({
				connection: validation.config,
				authorizationHeader: resolveAuthorizationHeader(
					authAuthorizationHeader,
					validation.config.devBearerToken,
				),
			});
			const nextProfiles = await client.listSyncProfiles();
			startTransition(() => {
				setProfiles(nextProfiles);
				setSelectedProfileId(nextProfiles[0]?.id ?? null);
			});
			setConnectionMessage(`Loaded ${nextProfiles.length} sync profile(s).`);
		} catch (error) {
			handleError(error);
		} finally {
			setIsLoading(false);
		}
	}

	async function handleCreateProfile() {
		await withClient(async (client) => {
			const nextProfile = await client.createSyncProfile({
				name: createProfileName,
				mode: "manual",
				defaultDirection: createProfileDirection,
				conflictPolicy: "manual",
			});
			replaceProfile(nextProfile);
			setSelectedProfileId(nextProfile.id);
			setConnectionMessage(`Created profile ${nextProfile.name}.`);
		});
	}

	async function handleSaveProfile() {
		if (!selectedProfile) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.updateSyncProfile({
				profileId: selectedProfile.id,
				input: {
					name: profileDraftName,
					enabled: profileDraftEnabled,
					defaultDirection: profileDraftDirection,
					conflictPolicy: profileDraftConflictPolicy,
				},
			});
			replaceProfile(nextProfile);
			setConnectionMessage(`Saved profile ${nextProfile.name}.`);
		});
	}

	async function handleCreateTarget() {
		if (!selectedProfile) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.createSyncProfileTarget({
				profileId: selectedProfile.id,
				input: targetDraft,
			});
			replaceProfile(nextProfile);
			setTargetDraft(defaultTargetDraft);
			setConnectionMessage(`Added target selector to ${nextProfile.name}.`);
		});
	}

	async function handleDeleteTarget(targetId: string) {
		if (!selectedProfile) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.deleteSyncProfileTarget({
				profileId: selectedProfile.id,
				targetId,
			});
			replaceProfile(nextProfile);
			setConnectionMessage(`Removed target selector from ${nextProfile.name}.`);
		});
	}

	async function handleCreateRule() {
		if (!selectedProfile) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.createSyncProfileRule({
				profileId: selectedProfile.id,
				input: newRuleDraft,
			});
			replaceProfile(nextProfile);
			setNewRuleDraft({
				ruleOrder: newRuleDraft.ruleOrder + 10,
				action: newRuleDraft.action,
				matcherType: newRuleDraft.matcherType,
				matcherValue: "",
			});
			setConnectionMessage(`Added rule to ${nextProfile.name}.`);
		});
	}

	async function handleSaveRule(ruleId: string) {
		if (!selectedProfile) {
			return;
		}
		const ruleDraft = ruleDrafts[ruleId];
		if (!ruleDraft) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.updateSyncProfileRule({
				profileId: selectedProfile.id,
				ruleId,
				input: ruleDraft,
			});
			replaceProfile(nextProfile);
			setConnectionMessage(`Updated rule ${ruleId.slice(0, 8)}.`);
		});
	}

	async function handleDeleteRule(ruleId: string) {
		if (!selectedProfile) {
			return;
		}
		await withClient(async (client) => {
			const nextProfile = await client.deleteSyncProfileRule({
				profileId: selectedProfile.id,
				ruleId,
			});
			replaceProfile(nextProfile);
			setConnectionMessage(`Removed rule ${ruleId.slice(0, 8)}.`);
		});
	}

	function replaceProfile(nextProfile: SyncProfileDetailView) {
		startTransition(() => {
			setProfiles((current) => {
				const existingIndex = current.findIndex(
					(profile) => profile.id === nextProfile.id,
				);
				if (existingIndex === -1) {
					return [...current, nextProfile];
				}
				const nextProfiles = [...current];
				nextProfiles[existingIndex] = nextProfile;
				return nextProfiles;
			});
		});
	}

	async function withClient(
		action: (
			client: ReturnType<typeof createDashboardApiClient>,
		) => Promise<void>,
	) {
		const validation = validateDashboardConnectionConfig(connectionConfig);
		if (!validation.isValid) {
			setConnectionMessage(validation.message);
			setLastError({
				code: "invalid_connection",
				message: validation.message,
				source: null,
			});
			return;
		}
		const authorizationHeader = resolveAuthorizationHeader(
			authAuthorizationHeader,
			validation.config.devBearerToken,
		);
		if (!authorizationHeader) {
			setConnectionMessage("Login is required before calling Dashboard APIs.");
			setLastError({
				code: "unauthenticated",
				message:
					"dashboard API calls require an authenticated token-set session or the advanced dev fallback token.",
				source: null,
			});
			return;
		}
		setIsLoading(true);
		setLastError(null);
		try {
			saveDashboardConnectionConfig(globalThis.localStorage, validation.config);
			await action(
				createDashboardApiClient({
					connection: validation.config,
					authorizationHeader,
				}),
			);
		} catch (error) {
			handleError(error);
		} finally {
			setIsLoading(false);
		}
	}

	function handleError(error: unknown) {
		const nextError = toDashboardErrorPayload(error);
		setLastError(nextError);
		setConnectionMessage(`${nextError.code}: ${nextError.message}`);
	}

	function handleLogin() {
		const validation = validateDashboardConnectionConfig(connectionConfig);
		if (!validation.isValid) {
			setConnectionMessage(validation.message);
			setLastError({
				code: "invalid_connection",
				message: validation.message,
				source: null,
			});
			return;
		}

		saveDashboardConnectionConfig(globalThis.localStorage, validation.config);
		saveDashboardPostAuthRedirect(globalThis.sessionStorage, "/");
		authClient.login({
			postAuthRedirectUri: new URL(
				authClient.paths.callbackPath,
				globalThis.location.origin,
			).toString(),
		});
	}

	async function handleLogout() {
		await authClient.logout();
		setAuthStatus("unauthenticated");
		setAuthAuthorizationHeader(null);
		setAuthDisplayName(null);
		setAuthSubject(null);
		setAuthMessage(
			hasDevBearerFallback
				? "Local token-set auth was cleared. Advanced dev bearer fallback remains configured."
				: "Local token-set auth was cleared.",
		);
		setProfiles([]);
		setSelectedProfileId(null);
		setConnectionMessage("Auth state cleared.");
		setLastError(null);
	}
}

function createDashboardAuthClient(config: {
	apiBaseUrl: string;
	oidcSource: string;
}) {
	return createAmagiAuthClient({
		baseUrl: config.apiBaseUrl,
		oidcSource: config.oidcSource,
		host: AmagiAuthHost.Dashboard,
	});
}

function resolveAuthorizationHeader(
	authAuthorizationHeader: string | null,
	devBearerToken: string,
) {
	if (authAuthorizationHeader) {
		return authAuthorizationHeader;
	}

	const trimmedToken = devBearerToken.trim();
	return trimmedToken ? `Bearer ${trimmedToken}` : null;
}

function readPrincipalDisplayName(
	snapshot: {
		metadata?: { principal?: { displayName?: unknown } };
	} | null,
) {
	return typeof snapshot?.metadata?.principal?.displayName === "string"
		? snapshot.metadata.principal.displayName
		: null;
}

function readPrincipalSubject(
	snapshot: {
		metadata?: { principal?: { subject?: unknown } };
	} | null,
) {
	return typeof snapshot?.metadata?.principal?.subject === "string"
		? snapshot.metadata.principal.subject
		: null;
}

function saveDashboardPostAuthRedirect(
	storage: Pick<Storage, "setItem">,
	path: string,
) {
	storage.setItem(DASHBOARD_POST_AUTH_REDIRECT_STORAGE_KEY, path);
}

function consumeDashboardPostAuthRedirect(
	storage: Pick<Storage, "getItem" | "removeItem">,
) {
	const path = storage.getItem(DASHBOARD_POST_AUTH_REDIRECT_STORAGE_KEY);
	if (path) {
		storage.removeItem(DASHBOARD_POST_AUTH_REDIRECT_STORAGE_KEY);
	}
	return path;
}

function toDashboardErrorPayload(error: unknown): DashboardApiErrorPayload {
	if (
		typeof error === "object" &&
		error !== null &&
		"code" in error &&
		typeof error.code === "string" &&
		"message" in error &&
		typeof error.message === "string"
	) {
		return {
			code: error.code,
			message: error.message,
			source:
				"source" in error && typeof error.source === "string"
					? error.source
					: null,
		};
	}
	return {
		code: "unknown_error",
		message: error instanceof Error ? error.message : String(error),
		source: null,
	};
}

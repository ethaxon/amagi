import { useEffect, useState } from "react";
import { browser } from "wxt/browser";

import {
	clearExtensionAuth,
	loadBrowserExtensionConfig,
	loadExtensionAuthState,
	saveBrowserExtensionConfig,
	startExtensionLogin,
} from "../extension/runtime";
import {
	defaultExtensionConfig,
	type ExtensionAuthState,
	type ExtensionConfig,
} from "../extension/state";

const defaultAuthState: ExtensionAuthState = {
	status: "unauthenticated",
	message: "Checking local extension auth state...",
	displayName: null,
	subject: null,
	usesDevBearerFallback: false,
};

export function OptionsApp() {
	const [config, setConfig] = useState<ExtensionConfig>(defaultExtensionConfig);
	const [authState, setAuthState] =
		useState<ExtensionAuthState>(defaultAuthState);
	const [status, setStatus] = useState("Loading options...");

	useEffect(() => {
		void loadBrowserExtensionConfig(browser)
			.then(async (nextConfig) => {
				setConfig(nextConfig);
				setAuthState(await loadExtensionAuthState(browser));
				setStatus("Loaded local extension config.");
			})
			.catch((error: unknown) => {
				setStatus(error instanceof Error ? error.message : String(error));
			});
	}, []);

	return (
		<main
			style={{
				fontFamily: "ui-sans-serif, system-ui",
				padding: 24,
				maxWidth: 640,
			}}
		>
			<h1 style={{ marginTop: 0 }}>Amagi Extension Options</h1>
			<p style={{ color: "#555" }}>
				This options page is the primary extension auth surface. Use the
				SecurityDept backend-oidc flow here, and keep the bearer token field as
				an advanced fallback only.
			</p>
			<section
				style={{
					border: "1px solid #ddd",
					borderRadius: 12,
					padding: 16,
					marginBottom: 20,
					background: "#faf8f3",
				}}
			>
				<div style={{ display: "grid", gap: 8 }}>
					<strong>Auth Status: {authState.status}</strong>
					<span style={{ color: "#555" }}>{authState.message}</span>
					{authState.displayName ? <span>{authState.displayName}</span> : null}
					{authState.subject ? (
						<code style={{ fontSize: 12 }}>{authState.subject}</code>
					) : null}
				</div>
				<div
					style={{ display: "flex", gap: 12, flexWrap: "wrap", marginTop: 16 }}
				>
					<button onClick={() => void handleLogin()} type="button">
						Login with Dex
					</button>
					<button onClick={() => void handleClearAuth()} type="button">
						Clear Auth
					</button>
				</div>
			</section>
			<form
				onSubmit={(event) => {
					event.preventDefault();
					void saveOptions();
				}}
				style={{ display: "grid", gap: 16 }}
			>
				<label>
					<div>API Base URL</div>
					<input
						onChange={(event) =>
							setConfig((current) => ({
								...current,
								apiBaseUrl: event.currentTarget.value,
							}))
						}
						style={{ width: "100%" }}
						value={config.apiBaseUrl}
					/>
				</label>
				<label>
					<div>OIDC Source</div>
					<input
						onChange={(event) =>
							setConfig((current) => ({
								...current,
								oidcSource: event.currentTarget.value,
							}))
						}
						style={{ width: "100%" }}
						value={config.oidcSource}
					/>
				</label>
				<details>
					<summary>Advanced Dev Fallback</summary>
					<div style={{ display: "grid", gap: 12, marginTop: 12 }}>
						<p style={{ color: "#555", margin: 0 }}>
							Only use this token field when debugging frontend auth issues. The
							normal happy path should use Login with Dex.
						</p>
						<label>
							<div>Dev Bearer Token Placeholder</div>
							<input
								onChange={(event) =>
									setConfig((current) => ({
										...current,
										devBearerToken: event.currentTarget.value,
									}))
								}
								style={{ width: "100%" }}
								value={config.devBearerToken}
							/>
						</label>
					</div>
				</details>
				<button style={{ width: 180, padding: 10 }} type="submit">
					Save Options
				</button>
			</form>
			<p style={{ color: "#666", marginTop: 16 }}>{status}</p>
		</main>
	);

	async function saveOptions() {
		try {
			await saveBrowserExtensionConfig(browser, config);
			setAuthState(await loadExtensionAuthState(browser));
			setStatus("Saved local extension config.");
		} catch (error: unknown) {
			setStatus(error instanceof Error ? error.message : String(error));
		}
	}

	async function handleLogin() {
		try {
			await startExtensionLogin(browser);
			setStatus("Opened the extension login flow in a new tab.");
		} catch (error: unknown) {
			setStatus(error instanceof Error ? error.message : String(error));
		}
	}

	async function handleClearAuth() {
		try {
			await clearExtensionAuth(browser);
			setAuthState(await loadExtensionAuthState(browser));
			setStatus("Cleared local extension auth state.");
		} catch (error: unknown) {
			setStatus(error instanceof Error ? error.message : String(error));
		}
	}
}

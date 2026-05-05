import { useEffect, useState } from "react";
import { browser } from "wxt/browser";

import { ExtensionMessageType } from "../extension/messaging";
import {
	clearExtensionAuth,
	loadExtensionAuthState,
	manualSyncResultToViewState,
	requestExtensionStatus,
	requestManualSync,
	startExtensionLogin,
} from "../extension/runtime";
import {
	defaultExtensionViewState,
	type ExtensionAuthState,
	ExtensionAuthStateStatus,
	type ExtensionViewState,
	ExtensionViewStateStatus,
} from "../extension/state";

const defaultAuthState: ExtensionAuthState = {
	status: ExtensionAuthStateStatus.Unauthenticated,
	message: "Checking local extension auth state...",
	displayName: null,
	subject: null,
	usesDevBearerFallback: false,
};

export function PopupApp() {
	const [state, setState] = useState<ExtensionViewState>(
		defaultExtensionViewState,
	);
	const [authState, setAuthState] =
		useState<ExtensionAuthState>(defaultAuthState);
	const [isPending, setIsPending] = useState(false);
	const canRunManualSync =
		authState.status === ExtensionAuthStateStatus.Authenticated ||
		authState.usesDevBearerFallback;

	useEffect(() => {
		void Promise.all([
			requestExtensionStatus(browser),
			loadExtensionAuthState(browser),
		])
			.then(([nextState, nextAuthState]) => {
				setState(nextState);
				setAuthState(nextAuthState);
			})
			.catch((error: unknown) => {
				setState({
					status: ExtensionViewStateStatus.Error,
					message: error instanceof Error ? error.message : String(error),
					previewSummary: null,
				});
			});
	}, []);

	return (
		<main
			style={{
				fontFamily: "ui-sans-serif, system-ui",
				padding: 16,
				width: 320,
			}}
		>
			<h1 style={{ margin: 0, fontSize: 18 }}>Amagi</h1>
			<section
				style={{
					marginTop: 12,
					padding: 12,
					borderRadius: 12,
					background: "#f5f2ea",
					border: "1px solid #ddd3c4",
				}}
			>
				<strong>Auth: {authState.status}</strong>
				<p style={{ color: "#444", fontSize: 13, margin: "8px 0 0" }}>
					{authState.message}
				</p>
				{authState.displayName ? (
					<p style={{ margin: "8px 0 0", fontSize: 13 }}>
						{authState.displayName}
					</p>
				) : null}
				{authState.subject ? (
					<code style={{ fontSize: 12 }}>{authState.subject}</code>
				) : null}
				<div style={{ display: "grid", gap: 8, marginTop: 12 }}>
					<button
						disabled={isPending}
						onClick={() => void handleLogin()}
						style={{ width: "100%", padding: 10 }}
						type="button"
					>
						Login with Dex
					</button>
					<button
						disabled={isPending}
						onClick={() => void handleClearAuth()}
						style={{ width: "100%", padding: 10 }}
						type="button"
					>
						Clear Auth
					</button>
				</div>
			</section>
			<p style={{ color: "#444", fontSize: 13 }}>{state.message}</p>
			<section>
				<strong>Status:</strong> {state.status}
			</section>
			<section style={{ marginTop: 12 }}>
				<strong>Preview Summary</strong>
				{state.previewSummary ? (
					<ul style={{ margin: "8px 0 0", paddingLeft: 18, fontSize: 13 }}>
						<li>Server to Local: {state.previewSummary.serverToLocal}</li>
						<li>
							Local Accepted: {state.previewSummary.localToServerAccepted}
						</li>
						<li>Conflicts: {state.previewSummary.conflicts}</li>
					</ul>
				) : (
					<p style={{ margin: "8px 0 0", fontSize: 13, color: "#666" }}>
						No pending preview cached in this shell build.
					</p>
				)}
			</section>
			<div style={{ display: "grid", gap: 8, marginTop: 16 }}>
				<button
					disabled={isPending || !canRunManualSync}
					onClick={() => void handleSyncAction(ExtensionMessageType.Preview)}
					style={{ width: "100%", padding: 10 }}
					type="button"
				>
					Preview Manual Sync
				</button>
				<button
					disabled={isPending || !canRunManualSync}
					onClick={() => void handleSyncAction(ExtensionMessageType.Apply)}
					style={{ width: "100%", padding: 10 }}
					type="button"
				>
					Apply Manual Sync
				</button>
			</div>
		</main>
	);

	async function handleSyncAction(
		type:
			| typeof ExtensionMessageType.Preview
			| typeof ExtensionMessageType.Apply,
	) {
		setIsPending(true);
		try {
			const response = await requestManualSync(browser, { type });
			setState(manualSyncResultToViewState(response));
			setAuthState(await loadExtensionAuthState(browser));
		} catch (error: unknown) {
			setState({
				status: ExtensionViewStateStatus.Error,
				message: error instanceof Error ? error.message : String(error),
				previewSummary: null,
			});
		} finally {
			setIsPending(false);
		}
	}

	async function handleLogin() {
		setIsPending(true);
		try {
			await startExtensionLogin(browser);
			setState((current) => ({
				...current,
				message: "Opened the extension login flow in a new tab.",
			}));
		} catch (error: unknown) {
			setState({
				status: ExtensionViewStateStatus.Error,
				message: error instanceof Error ? error.message : String(error),
				previewSummary: null,
			});
		} finally {
			setIsPending(false);
		}
	}

	async function handleClearAuth() {
		setIsPending(true);
		try {
			await clearExtensionAuth(browser);
			setAuthState(await loadExtensionAuthState(browser));
			setState((current) => ({
				...current,
				message: "Cleared local extension auth state.",
			}));
		} catch (error: unknown) {
			setState({
				status: ExtensionViewStateStatus.Error,
				message: error instanceof Error ? error.message : String(error),
				previewSummary: null,
			});
		} finally {
			setIsPending(false);
		}
	}
}

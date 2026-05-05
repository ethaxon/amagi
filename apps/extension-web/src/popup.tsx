import { createRoot } from "react-dom/client";

import { defaultExtensionViewState } from "./state";

function PopupShell() {
	const state = defaultExtensionViewState;
	return (
		<main
			style={{
				fontFamily: "ui-sans-serif, system-ui",
				padding: 16,
				width: 320,
			}}
		>
			<h1 style={{ margin: 0, fontSize: 18 }}>Amagi</h1>
			<p style={{ color: "#444", fontSize: 13 }}>{state.message}</p>
			<section>
				<strong>Status:</strong> {state.status}
			</section>
			<section style={{ marginTop: 12 }}>
				<strong>Preview Summary</strong>
				<p style={{ margin: "8px 0 0", fontSize: 13, color: "#666" }}>
					No pending preview cached in this shell build.
				</p>
			</section>
			<button
				style={{ marginTop: 16, width: "100%", padding: 10 }}
				type="button"
			>
				Manual Sync
			</button>
		</main>
	);
}

const rootNode = document.getElementById("root");
if (rootNode) {
	createRoot(rootNode).render(<PopupShell />);
}

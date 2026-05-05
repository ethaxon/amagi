import { createRoot } from "react-dom/client";

import { defaultExtensionConfig } from "./state";

function OptionsShell() {
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
				This iteration only ships a dev-only configuration shell. It does not
				promise secure long-term bearer token storage.
			</p>
			<form style={{ display: "grid", gap: 16 }}>
				<label>
					<div>API Base URL</div>
					<input
						defaultValue={defaultExtensionConfig.apiBaseUrl}
						style={{ width: "100%" }}
					/>
				</label>
				<label>
					<div>OIDC Source</div>
					<input
						defaultValue={defaultExtensionConfig.oidcSource}
						style={{ width: "100%" }}
					/>
				</label>
				<label>
					<div>Dev Bearer Token Placeholder</div>
					<input
						defaultValue={defaultExtensionConfig.devBearerToken}
						style={{ width: "100%" }}
					/>
				</label>
			</form>
		</main>
	);
}

const rootNode = document.getElementById("root");
if (rootNode) {
	createRoot(rootNode).render(<OptionsShell />);
}

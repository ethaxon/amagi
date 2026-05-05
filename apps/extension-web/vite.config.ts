import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

import { manifestDefinition } from "./src/manifest.ts";

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), "");
	const backendUrl = env.VITE_BACKEND_URL || "http://localhost:7021";

	return {
		plugins: [
			react(),
			tailwindcss(),
			{
				name: "amagi-extension-manifest",
				generateBundle() {
					this.emitFile({
						type: "asset",
						fileName: "manifest.json",
						source: `${JSON.stringify(manifestDefinition, null, 2)}\n`,
					});
				},
			},
		],
		resolve: {
			conditions: ["monorepo-tsc"],
			alias: {
				"@ethaxon/amagi-sync-client": resolve(
					__dirname,
					"../../packages/amagi-sync-client/src/index.ts",
				),
				"@ethaxon/amagi-browser-adapter-chromium": resolve(
					__dirname,
					"../../packages/amagi-browser-adapter-chromium/src/index.ts",
				),
				"@": resolve(__dirname, "./src"),
			},
		},
		server: {
			proxy: {
				"^/auth/(?!token-set/frontend-mode/(?:callback|popup-callback)(?:\\?.*)?$)":
					{
						target: backendUrl,
						// for local development server use forwarded header as callback url
						headers: {
							Forwarded: "for=127.0.0.1;proto=http;host=localhost:7022",
						},
					},
				"/api": {
					target: backendUrl,
					// for local development server use forwarded header as callback url
					headers: {
						Forwarded: "for=127.0.0.1;proto=http;host=localhost:7022",
					},
				},
				"/basic": {
					target: backendUrl,
					headers: {
						Forwarded: "for=127.0.0.1;proto=http;host=localhost:7022",
					},
				},
			},
			port: 7022,
		},
		build: {
			outDir: "dist",
			emptyOutDir: true,
			rollupOptions: {
				input: {
					background: resolve(__dirname, "./src/background.ts"),
					popup: resolve(__dirname, "./popup.html"),
					options: resolve(__dirname, "./options.html"),
				},
				output: {
					entryFileNames(chunkInfo) {
						if (chunkInfo.name === "background") {
							return "background.js";
						}
						return "assets/[name]-[hash].js";
					},
					chunkFileNames: "assets/[name]-[hash].js",
					assetFileNames: "assets/[name]-[hash][extname]",
				},
			},
		},
		appType: "mpa",
	};
});

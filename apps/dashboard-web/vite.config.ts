import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
	resolve: {
		alias: {
			"@ethaxon/amagi-auth-client": path.resolve(
				import.meta.dirname,
				"../../packages/amagi-auth-client/src/index.ts",
			),
		},
	},
	plugins: [react()],
	server: {
		port: 4174,
	},
});

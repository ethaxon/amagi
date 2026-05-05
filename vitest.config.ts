import path from "node:path";
import { defineConfig } from "vitest/config";

const projectRoot = path.resolve(import.meta.dirname);
const ciTestTimeoutMs = 15_000;
const isCi = process.env.CI === "true" || process.env.GITHUB_ACTIONS === "true";

export default defineConfig({
	resolve: {
		conditions: ["monorepo-tsc"],
		alias: [
			{
				find: "@ethaxon/amagi-auth-client",
				replacement: path.resolve(
					projectRoot,
					"packages/amagi-auth-client/src/index.ts",
				),
			},
			{
				find: "@ethaxon/amagi-sync-client",
				replacement: path.resolve(
					projectRoot,
					"packages/amagi-sync-client/src/index.ts",
				),
			},
			{
				find: "@ethaxon/amagi-webext",
				replacement: path.resolve(
					projectRoot,
					"packages/amagi-webext/src/index.ts",
				),
			},
			{
				find: "@ethaxon/amagi-dashboard-web",
				replacement: path.resolve(
					projectRoot,
					"apps/dashboard-web/src/index.ts",
				),
			},
		],
	},
	test: {
		environment: "node",
		include: [
			"apps/**/__tests__/**/*.{ts,tsx}",
			"packages/**/__tests__/**/*.{ts,tsx}",
			"!**/node_modules/**",
			"!**/dist/**",
			"!**/dist-tsc/**",
		],
		testTimeout: isCi ? ciTestTimeoutMs : undefined,
	},
});

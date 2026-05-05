import fs from "node:fs";
import { platform } from "node:os";
import path from "node:path";
import type { AutoIconsOptions } from "@wxt-dev/auto-icons";
import { defineConfig, type UserConfig } from "wxt";

const repositoryRoot = path.resolve(import.meta.dirname, "../..");
const platformName = platform();

export default defineConfig({
	modules: ["@wxt-dev/module-react", "@wxt-dev/auto-icons"],
	alias: {
		"@ethaxon/amagi-auth-client": path.resolve(
			repositoryRoot,
			"packages/amagi-auth-client/src/index.ts",
		),
		"@ethaxon/amagi-sync-client": path.resolve(
			repositoryRoot,
			"packages/amagi-sync-client/src/index.ts",
		),
		"@ethaxon/amagi-webext": path.resolve(
			repositoryRoot,
			"packages/amagi-webext/src/index.ts",
		),
	},
	hooks: {
		"build:before": () => {
			if (!fs.existsSync(path.join(import.meta.dirname, ".wxt/chrome-data"))) {
				fs.mkdirSync(path.join(import.meta.dirname, ".wxt/chrome-data"), {
					recursive: true,
				});
			}
		},
	},
	autoIcons: {
		enabled: true,
		developmentIndicator: "overlay",
		baseIconPath: path.resolve(
			import.meta.dirname,
			"../../assets/icons/cel/icon-1024.png",
		),
	},
	manifest: () => ({
		name: "Amagi",
		description: "Manual bookmark sync shell for Amagi.",
		permissions: ["bookmarks", "storage"],
		host_permissions: ["http://localhost/*", "http://127.0.0.1/*"],
		web_accessible_resources: [
			{
				resources: ["options.html"],
				matches: ["http://localhost/*", "http://127.0.0.1/*"],
			},
		],
	}),
	suppressWarnings: {
		firefoxDataCollection: false,
	},
	webExt: {
		// https://wxt.dev/guide/essentials/config/browser-startup.html#persist-data
		...(platformName === "win32"
			? {
					chromiumProfile: path.resolve(".wxt/chrome-data"),
					keepProfileChanges: true,
				}
			: {
					chromiumArgs: ["--user-data-dir=./.wxt/chrome-data"],
				}),
	},
} satisfies UserConfig & {
	autoIcons?: AutoIconsOptions;
});

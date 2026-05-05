export const manifestDefinition = {
	manifest_version: 3,
	name: "Amagi",
	version: "0.0.1",
	description: "Manual bookmark sync shell for Amagi.",
	permissions: ["bookmarks", "storage"],
	host_permissions: ["http://localhost:3000/*", "https://localhost:3000/*"],
	background: {
		service_worker: "background.js",
		type: "module",
	},
	action: {
		default_title: "Amagi",
		default_popup: "popup.html",
	},
	options_page: "options.html",
} as const;

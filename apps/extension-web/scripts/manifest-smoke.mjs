import fs from "node:fs";
import path from "node:path";

const target = process.argv[2];
const TARGET_OUTPUT_DIRECTORIES = {
	chrome: "chrome-mv3",
	firefox: "firefox-mv2",
	safari: "safari-mv2",
};

if (!target) {
	throw new Error("expected target argument: chrome, firefox, or safari");
}

const outputRoot = path.resolve(import.meta.dirname, "../.output");
const targetDirectory = findTargetDirectory(outputRoot, target);
const manifestPath = path.join(targetDirectory, "manifest.json");

if (!fs.existsSync(manifestPath)) {
	throw new Error(
		`manifest.json not found for ${target} build at ${manifestPath}`,
	);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const permissions = manifest.permissions ?? [];
const hostPermissions = manifest.host_permissions ?? [];
const action = manifest.action ?? manifest.browser_action ?? null;

for (const permission of ["bookmarks", "storage"]) {
	if (!permissions.includes(permission)) {
		throw new Error(`missing ${permission} permission in ${manifestPath}`);
	}
}

if (hostPermissions.includes("<all_urls>")) {
	throw new Error(`unexpected <all_urls> host permission in ${manifestPath}`);
}

if (!action?.default_popup) {
	throw new Error(`missing action.default_popup in ${manifestPath}`);
}

console.log(`manifest smoke check passed for ${target}: ${manifestPath}`);

function findTargetDirectory(outputRoot, target) {
	if (!fs.existsSync(outputRoot)) {
		throw new Error(`WXT output directory does not exist: ${outputRoot}`);
	}
	const outputDirectoryName = TARGET_OUTPUT_DIRECTORIES[target];
	if (!outputDirectoryName) {
		throw new Error(`unsupported target ${target}`);
	}
	return path.join(outputRoot, outputDirectoryName);
}

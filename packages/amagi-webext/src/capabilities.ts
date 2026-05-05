import type { BrowserFamily, BrowserLike, WebExtCapabilities } from "./types";

export async function detectWebExtCapabilities(
	browserLike: BrowserLike,
): Promise<WebExtCapabilities> {
	const browserFamily = await detectBrowserFamily(browserLike);
	const manifestVersion = readManifestVersion(browserLike);
	return {
		canReadBookmarks: typeof browserLike.bookmarks?.getTree === "function",
		canWriteBookmarks:
			typeof browserLike.bookmarks?.create === "function" &&
			typeof browserLike.bookmarks?.update === "function" &&
			typeof browserLike.bookmarks?.move === "function" &&
			(typeof browserLike.bookmarks?.remove === "function" ||
				typeof browserLike.bookmarks?.removeTree === "function"),
		canUseStorage:
			typeof browserLike.storage?.local?.get === "function" &&
			typeof browserLike.storage?.local?.set === "function",
		browserFamily,
		manifestVersion,
	};
}

async function detectBrowserFamily(
	browserLike: BrowserLike,
): Promise<BrowserFamily> {
	const browserInfo = await browserLike.runtime
		?.getBrowserInfo?.()
		.catch(() => null);
	const name = browserInfo?.name?.toLowerCase();
	if (name?.includes("firefox")) {
		return "firefox";
	}
	if (name?.includes("safari")) {
		return "safari";
	}
	if (name?.includes("chrome")) {
		return "chrome";
	}
	return browserLike.chrome ? "chrome" : "unknown";
}

function readManifestVersion(browserLike: BrowserLike): number | undefined {
	return (
		browserLike.runtime?.getManifest?.()?.manifest_version ??
		browserLike.chrome?.runtime?.getManifest?.()?.manifest_version
	);
}

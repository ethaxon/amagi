import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { chromium } from "playwright";

const extensionPath = path.resolve(
	import.meta.dirname,
	"../.output/chrome-mv3",
);
const manifestPath = path.join(extensionPath, "manifest.json");

if (!fs.existsSync(manifestPath)) {
	throw new Error(
		`Chrome extension build output is missing at ${manifestPath}. Run "pnpm build:chrome" first.`,
	);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const popupPath =
	manifest.action?.default_popup ?? manifest.browser_action?.default_popup;

if (!popupPath || typeof popupPath !== "string") {
	throw new Error(
		`manifest is missing action.default_popup at ${manifestPath}`,
	);
}

const normalizedPopupPath = popupPath.replace(/^\//u, "");
const userDataDir = fs.mkdtempSync(
	path.join(os.tmpdir(), "amagi-extension-smoke-"),
);

let context;

try {
	context = await chromium.launchPersistentContext(userDataDir, {
		channel: "chromium",
		headless: false,
		args: [
			`--disable-extensions-except=${extensionPath}`,
			`--load-extension=${extensionPath}`,
		],
	});

	const serviceWorker =
		context.serviceWorkers()[0] ??
		(await context.waitForEvent("serviceworker", { timeout: 15_000 }));
	const extensionId = new URL(serviceWorker.url()).host;
	const popupPage = await context.newPage();
	const pageErrors = [];
	const consoleMessages = [];

	popupPage.on("pageerror", (error) => {
		pageErrors.push(error.message);
	});
	popupPage.on("console", (message) => {
		consoleMessages.push(`${message.type()}: ${message.text()}`);
	});

	await popupPage.goto(
		`chrome-extension://${extensionId}/${normalizedPopupPath}`,
	);
	await popupPage.waitForLoadState("domcontentloaded");
	try {
		await popupPage.waitForFunction(
			() => Boolean(document.body?.textContent?.trim()),
			null,
			{ timeout: 15_000 },
		);
	} catch (error) {
		const rootHtml = await popupPage
			.locator("#root")
			.innerHTML()
			.catch(() => "<missing-root>");
		throw new Error(
			[
				"popup page did not render text content",
				`url=${popupPage.url()}`,
				`rootHtml=${rootHtml}`,
				`pageErrors=${pageErrors.join(" | ") || "<none>"}`,
				`console=${consoleMessages.join(" | ") || "<none>"}`,
				`cause=${error instanceof Error ? error.message : String(error)}`,
			].join("\n"),
		);
	}

	const bodyText = (await popupPage.textContent("body"))?.trim() ?? "";
	if (!bodyText) {
		throw new Error("popup page rendered an empty body");
	}

	console.log(
		`chrome extension smoke passed: extensionId=${extensionId} popup=${normalizedPopupPath}`,
	);
} catch (error) {
	if (
		error instanceof Error &&
		error.message.includes("Executable doesn't exist")
	) {
		throw new Error(
			`${error.message}\nRun "pnpm --filter @ethaxon/amagi-extension-web exec playwright install chromium" and retry.`,
		);
	}
	throw error;
} finally {
	await context?.close();
	fs.rmSync(userDataDir, { recursive: true, force: true });
}

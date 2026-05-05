import { browser } from "wxt/browser";

import { createBackgroundRuntime } from "../src/extension/runtime";

export default defineBackground(() => {
	const runtime = createBackgroundRuntime({ browser });
	browser.runtime.onMessage.addListener(
		(message: unknown, _sender, sendResponse: (response: unknown) => void) => {
			void runtime
				.handleMessage(message)
				.then((response) => sendResponse(response))
				.catch((error: unknown) => {
					sendResponse({
						status: "error",
						message: error instanceof Error ? error.message : String(error),
						previewSummary: null,
					});
				});

			return true;
		},
	);
});

import { createRoot } from "react-dom/client";

import { PopupApp } from "../../src/ui/popup";

const rootNode = document.getElementById("root");

if (rootNode) {
	createRoot(rootNode).render(<PopupApp />);
}

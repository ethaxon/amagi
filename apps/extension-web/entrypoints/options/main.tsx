import { createRoot } from "react-dom/client";

import { OptionsApp } from "../../src/ui/options";

const rootNode = document.getElementById("root");

if (rootNode) {
	createRoot(rootNode).render(<OptionsApp />);
}

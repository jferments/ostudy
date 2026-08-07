import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { recordWebviewDiagnostic } from "./diagnostics";
import "./styles.css";
import "./practice-confidence.css";
import "./card-organization.css";
import "./card-types.css";
import "./rich-templates.css";
import "./scheduling-controls.css";
import "./analytics.css";
import "./managed-assets.css";

window.addEventListener("error", (event) => {
  void recordWebviewDiagnostic(
    "webview_uncaught_error",
    event.error instanceof Error ? event.error : event.message,
  );
});
window.addEventListener("unhandledrejection", (event) => {
  void recordWebviewDiagnostic("webview_unhandled_rejection", event.reason);
});

const rootElement = document.getElementById("root");

if (rootElement === null) {
  throw new Error("The application root element is missing.");
}

createRoot(rootElement).render(
  <StrictMode>
    <AppErrorBoundary>
      <App />
    </AppErrorBoundary>
  </StrictMode>,
);

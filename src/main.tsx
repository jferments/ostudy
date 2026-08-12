import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { recordWebviewDiagnostic } from "./ipc/diagnostics";
import { ExperienceProvider } from "./ui/feedback/ExperienceProvider";
import "./ui/foundations/index.css";
import { OverlayProvider } from "./ui/overlays/OverlayProvider";
import { PlatformProvider } from "./platform/PlatformProvider";

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
    <PlatformProvider>
      <OverlayProvider>
      <ExperienceProvider>
        <AppErrorBoundary>
          <App />
        </AppErrorBoundary>
      </ExperienceProvider>
      </OverlayProvider>
    </PlatformProvider>
  </StrictMode>,
);

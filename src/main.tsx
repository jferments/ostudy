import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { PlatformProvider } from "./platform/PlatformProvider";
import { ExperienceProvider } from "./ui/feedback/ExperienceProvider";
import "./ui/foundations/index.css";
import { HelpProvider } from "./ui/help/HelpProvider";
import { OverlayProvider } from "./ui/overlays/OverlayProvider";

const root = document.getElementById("root");
if (!root) throw new Error("The ostudy root element is missing.");

if (import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE === "1") {
  const documentRoot = document.documentElement;
  documentRoot.dataset.androidTestBridge = "loading";
  void import("./testing/androidTestBridge")
    .then(({ installAndroidTestBridge }) => {
      installAndroidTestBridge();
      const smokeWindow = window as Window & { __OSTUDY_ANDROID_TEST__?: unknown };
      documentRoot.dataset.androidTestBridge =
        typeof smokeWindow.__OSTUDY_ANDROID_TEST__ === "object" ? "ready" : "missing";
    })
    .catch((error: unknown) => {
      documentRoot.dataset.androidTestBridge = "error";
      documentRoot.dataset.androidTestBridgeError =
        error instanceof Error ? error.message : String(error);
      console.error("Unable to install the Android smoke test bridge:", error);
    });
}

createRoot(root).render(
  <StrictMode>
    <PlatformProvider>
      <OverlayProvider>
        <HelpProvider>
          <ExperienceProvider>
            <AppErrorBoundary>
              <App />
            </AppErrorBoundary>
          </ExperienceProvider>
        </HelpProvider>
      </OverlayProvider>
    </PlatformProvider>
  </StrictMode>,
);

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

type AndroidTestBridgeDiagnosticState = "loading" | "ready" | "missing" | "error";

function setAndroidTestBridgeDiagnostic(
  state: AndroidTestBridgeDiagnosticState,
  error?: unknown,
): void {
  const apply = () => {
    const documentRoot = document.documentElement;
    if (!documentRoot) return false;
    documentRoot.dataset.androidTestBridge = state;
    if (error === undefined) {
      delete documentRoot.dataset.androidTestBridgeError;
    } else {
      documentRoot.dataset.androidTestBridgeError =
        error instanceof Error ? error.message : String(error);
    }
    return true;
  };

  if (!apply()) {
    document.addEventListener("DOMContentLoaded", () => {
      apply();
    }, { once: true });
  }
}

if (import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE === "1") {
  setAndroidTestBridgeDiagnostic("loading");
  void import("./testing/androidTestBridge")
    .then(({ installAndroidTestBridge }) => {
      installAndroidTestBridge();
      const smokeWindow = window as Window & { __OSTUDY_ANDROID_TEST__?: unknown };
      setAndroidTestBridgeDiagnostic(
        typeof smokeWindow.__OSTUDY_ANDROID_TEST__ === "object" ? "ready" : "missing",
      );
    })
    .catch((error: unknown) => {
      setAndroidTestBridgeDiagnostic("error", error);
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

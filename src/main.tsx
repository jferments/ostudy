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

let androidTestBridgeDiagnostic:
  | { state: AndroidTestBridgeDiagnosticState; error?: unknown }
  | null = null;
let androidTestBridgeDiagnosticScheduled = false;

function setAndroidTestBridgeDiagnostic(
  state: AndroidTestBridgeDiagnosticState,
  error?: unknown,
): void {
  androidTestBridgeDiagnostic = { state, error };

  const apply = () => {
    const documentRoot = document.documentElement;
    const diagnostic = androidTestBridgeDiagnostic;
    if (!documentRoot || !diagnostic) return false;
    documentRoot.dataset.androidTestBridge = diagnostic.state;
    if (diagnostic.error === undefined) {
      delete documentRoot.dataset.androidTestBridgeError;
    } else {
      documentRoot.dataset.androidTestBridgeError =
        diagnostic.error instanceof Error ? diagnostic.error.message : String(diagnostic.error);
    }
    return true;
  };

  if (apply() || androidTestBridgeDiagnosticScheduled) return;
  androidTestBridgeDiagnosticScheduled = true;
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      androidTestBridgeDiagnosticScheduled = false;
      apply();
    },
    { once: true },
  );
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
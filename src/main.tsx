import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { installGlobalWebviewDiagnostics, queueFrontendDiagnostic } from "./ipc/diagnostics";
import { PlatformProvider } from "./platform/PlatformProvider";
import { ExperienceProvider } from "./ui/feedback/ExperienceProvider";
import "./ui/foundations/index.css";
import { HelpProvider } from "./ui/help/HelpProvider";
import { OverlayProvider } from "./ui/overlays/OverlayProvider";

installGlobalWebviewDiagnostics();
queueFrontendDiagnostic({
  level: "debug",
  target: "frontend.bootstrap",
  event: "webview_bootstrap_started",
  message: "The React webview bootstrap started.",
  fields: { strictMode: true },
});

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
      const state =
        typeof smokeWindow.__OSTUDY_ANDROID_TEST__ === "object" ? "ready" : "missing";
      setAndroidTestBridgeDiagnostic(state);
      queueFrontendDiagnostic({
        level: state === "ready" ? "debug" : "warn",
        target: "frontend.android_test_bridge",
        event: "android_test_bridge_loaded",
        message: "The Android smoke-test bridge finished loading.",
        fields: { state },
      });
    })
    .catch((error: unknown) => {
      setAndroidTestBridgeDiagnostic("error", error);
      queueFrontendDiagnostic({
        level: "error",
        target: "frontend.android_test_bridge",
        event: "android_test_bridge_failed",
        message: "The Android smoke-test bridge could not be installed.",
        fields: {
          errorType: error instanceof Error ? error.name : typeof error,
        },
      });
    });
}

if (
  typeof __OSTUDY_UI_PERFORMANCE_CAPTURE__ !== "undefined" &&
  __OSTUDY_UI_PERFORMANCE_CAPTURE__
) {
  void import("./testing/uiPerformanceBridge").then(({ installUiPerformanceBridge }) => {
    installUiPerformanceBridge();
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

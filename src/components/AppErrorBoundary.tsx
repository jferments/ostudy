import { Component, type ErrorInfo, type ReactNode } from "react";

import { recordWebviewDiagnostic } from "../ipc/diagnostics";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  error: Error | null;
}

/**
 * Prevents a rendering exception in one screen from turning the entire Tauri
 * window into an unexplained blank surface. The boundary deliberately keeps
 * the recovery UI free of native capabilities: reloading reconstructs React
 * state while durable data remains in SQLite.
 */
export class AppErrorBoundary extends Component<
  AppErrorBoundaryProps,
  AppErrorBoundaryState
> {
  state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    void recordWebviewDiagnostic(
      "webview_render_error",
      error,
      info.componentStack ?? undefined,
    );
  }

  render(): ReactNode {
    if (!this.state.error) return this.props.children;

    return (
      <section className="fatal-screen embedded-fatal-screen" role="alert">
        <h1>This screen could not be displayed</h1>
        <p>{this.state.error.message || "An unexpected interface error occurred."}</p>
        <button
          type="button"
          className="primary-button"
          onClick={() => window.location.reload()}
        >
          Reload ostudy
        </button>
      </section>
    );
  }
}

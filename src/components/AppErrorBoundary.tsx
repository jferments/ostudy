import { Component, type ErrorInfo, type ReactNode } from "react";

import { recordWebviewDiagnostic } from "../ipc/diagnostics";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";

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
      <section className="tw:mx-auto tw:grid tw:max-w-2xl tw:gap-5 tw:p-8">
        <h1 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">This screen could not be displayed</h1>
        <InlineNotice kind="error">{this.state.error.message || "An unexpected interface error occurred."}</InlineNotice>
        <Button variant="primary" onClick={() => window.location.reload()}>
          Reload ostudy
        </Button>
      </section>
    );
  }
}

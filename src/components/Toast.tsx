import { useEffect, useState } from "react";

export interface ToastAction {
  label: string;
  run: () => void | Promise<void>;
  expiresAt?: number;
}

export interface ToastMessage {
  id: number;
  kind: "success" | "error" | "info";
  text: string;
  action?: ToastAction;
}

interface ToastProps {
  toast: ToastMessage | null;
  onDismiss: () => void;
}

export function Toast({ toast, onDismiss }: ToastProps) {
  const [isRunningAction, setIsRunningAction] = useState(false);

  useEffect(() => {
    setIsRunningAction(false);
    if (!toast) return;
    const defaultDuration = toast.action ? 10_000 : toast.kind === "error" ? 7_000 : 4_500;
    const duration = toast.action?.expiresAt
      ? Math.max(0, Math.min(defaultDuration, toast.action.expiresAt - Date.now()))
      : defaultDuration;
    const timer = window.setTimeout(onDismiss, duration);
    return () => window.clearTimeout(timer);
  }, [toast, onDismiss]);

  if (!toast) return null;

  async function runAction(): Promise<void> {
    if (!toast?.action || isRunningAction) return;
    setIsRunningAction(true);
    try {
      await toast.action.run();
      onDismiss();
    } catch {
      // The action owner reports its domain-specific error. Leaving the action
      // enabled allows an intentional retry while the recovery window remains.
      setIsRunningAction(false);
    }
  }

  return (
    <div
      className={`toast toast-${toast.kind}`}
      role={toast.kind === "error" ? "alert" : "status"}
    >
      <span>{toast.text}</span>
      {toast.action ? (
        <button
          type="button"
          className="toast-action"
          disabled={isRunningAction}
          onClick={() => void runAction()}
        >
          {isRunningAction ? "Restoring…" : toast.action.label}
        </button>
      ) : null}
      <button type="button" aria-label="Dismiss notification" onClick={onDismiss}>
        ×
      </button>
    </div>
  );
}

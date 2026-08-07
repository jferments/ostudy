import { useEffect, useRef, useState } from "react";

import styles from "./Toast.module.css";

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

export type ShowToast = (
  kind: ToastMessage["kind"],
  text: string,
  action?: ToastAction,
) => void;

interface ToastProps {
  toast: ToastMessage | null;
  onDismiss: () => void;
}

export function Toast({ toast, onDismiss }: ToastProps) {
  const [isRunningAction, setIsRunningAction] = useState(false);
  const [paused, setPaused] = useState(false);
  const toastRef = useRef<HTMLDivElement | null>(null);
  const activeToastIdRef = useRef<number | null>(null);
  const deadlineRef = useRef(0);

  useEffect(() => {
    setIsRunningAction(false);
    setPaused(false);
    activeToastIdRef.current = toast?.id ?? null;
    if (!toast) {
      deadlineRef.current = 0;
      return;
    }
    const defaultDuration = toast.action ? 10_000 : toast.kind === "error" ? 7_000 : 4_500;
    const duration = toast.action?.expiresAt
      ? Math.max(0, Math.min(defaultDuration, toast.action.expiresAt - Date.now()))
      : defaultDuration;
    deadlineRef.current = Date.now() + duration;
  }, [toast]);

  useEffect(() => {
    if (!toast || paused) return;
    const remaining = Math.max(0, deadlineRef.current - Date.now());
    const timer = window.setTimeout(onDismiss, remaining);
    return () => window.clearTimeout(timer);
  }, [onDismiss, paused, toast]);

  if (!toast) return null;

  async function runAction(): Promise<void> {
    if (!toast?.action || isRunningAction) return;
    setIsRunningAction(true);
    try {
      await toast.action.run();
      onDismiss();
    } catch {
      setIsRunningAction(false);
    }
  }

  return (
    <div
      ref={toastRef}
      className={`${styles.toast} ${styles[toast.kind]}`}
      role={toast.kind === "error" ? "alert" : "status"}
      onPointerEnter={() => setPaused(true)}
      onPointerLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={(event) => {
        const next = event.relatedTarget;
        if (!(next instanceof Node) || !toastRef.current?.contains(next)) setPaused(false);
      }}
    >
      <span className={styles.message}>{toast.text}</span>
      {toast.action ? (
        <button
          type="button"
          className={styles.action}
          disabled={isRunningAction}
          onClick={() => void runAction()}
        >
          {isRunningAction ? "Restoring…" : toast.action.label}
        </button>
      ) : null}
      <button
        type="button"
        className={styles.dismiss}
        aria-label="Dismiss notification"
        onClick={onDismiss}
      >
        ×
      </button>
    </div>
  );
}

import { useEffect, useRef, useState } from "react";
import { NotificationView } from "../vendor/shadcn/notification";
import { Button } from "./Button";

export interface NotificationAction {
  label: string;
  run: () => void | Promise<void>;
  expiresAt?: number;
}

export interface NotificationMessage {
  id: number;
  kind: "success" | "error" | "info";
  text: string;
  action?: NotificationAction;
}

const ACTION_TOAST_DURATION_MS = 10_000;

/** App-owned action lifetime and absolute undo expiry; Base UI owns transient UI. */
export function Notification(props: { message: NotificationMessage; onDismiss: () => void }) {
  return <NotificationInstance key={props.message.id} {...props} />;
}

function NotificationInstance({ message, onDismiss }: { message: NotificationMessage; onDismiss: () => void }) {
  const [running, setRunning] = useState(false);
  const active = useRef(true);
  const inFlight = useRef(false);
  const dismissAfterAction = useRef(false);
  const dismissed = useRef(false);
  const dismiss = useRef(onDismiss);
  dismiss.current = onDismiss;
  const action = message.action;
  const expiresAt = action?.expiresAt;
  const [timeout] = useState(() => {
    const duration = action ? ACTION_TOAST_DURATION_MS : message.kind === "error" ? 7_000 : 4_500;
    return expiresAt !== undefined && Number.isFinite(expiresAt)
      ? Math.max(1, Math.min(duration, expiresAt - Date.now()))
      : duration;
  });

  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  function dismissOnce() {
    if (!active.current || dismissed.current) return;
    dismissed.current = true;
    dismiss.current();
  }

  useEffect(() => {
    if (!action) return;
    const timer = window.setTimeout(() => {
      if (inFlight.current) {
        dismissAfterAction.current = true;
        return;
      }
      dismissOnce();
    }, timeout);
    return () => window.clearTimeout(timer);
  }, [action, timeout]);

  async function runAction() {
    if (!action || inFlight.current) return;
    if (expiresAt !== undefined && expiresAt <= Date.now()) {
      dismissOnce();
      return;
    }
    inFlight.current = true;
    setRunning(true);
    try {
      await action.run();
      dismissOnce();
    } catch {
      inFlight.current = false;
      if (dismissAfterAction.current || (expiresAt !== undefined && expiresAt <= Date.now())) {
        dismissOnce();
      } else if (active.current) {
        setRunning(false);
      }
    }
  }

  return (
    <NotificationView
      id={String(message.id)}
      message={message.text}
      tone={message.kind}
      timeout={timeout}
      busy={running}
      onDismiss={dismissOnce}
      action={
        action ? (
          <Button
            size="small"
            variant="primary"
            disabled={running}
            onClick={() => void runAction()}
          >
            {running ? "Restoring…" : action.label}
          </Button>
        ) : undefined
      }
    />
  );
}

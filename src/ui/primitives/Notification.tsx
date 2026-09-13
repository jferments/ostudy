import { useEffect, useRef, useState } from "react";
import { NotificationView } from "../vendor/shadcn/notification";
import { Button } from "./Button";
export interface NotificationAction { label: string; run: () => void | Promise<void>; expiresAt?: number; }
export interface NotificationMessage { id: number; kind: "success" | "error" | "info"; text: string; action?: NotificationAction; }

/** App-owned action lifetime and absolute undo expiry; Base UI owns transient UI. */
export function Notification(props: { message: NotificationMessage; onDismiss: () => void }) {
  return <NotificationInstance key={props.message.id} {...props} />;
}
function NotificationInstance({ message, onDismiss }: { message: NotificationMessage; onDismiss: () => void }) {
  const [running, setRunning] = useState(false);
  const [expired, setExpired] = useState(false);
  const active = useRef(true);
  const inFlight = useRef(false);
  const dismissed = useRef(false);
  const dismiss = useRef(onDismiss); dismiss.current = onDismiss;
  const action = message.action;
  const expiresAt = action?.expiresAt;
  const [timeout] = useState(() => {
    const duration = action ? 10_000 : message.kind === "error" ? 7_000 : 4_500;
    return expiresAt !== undefined && Number.isFinite(expiresAt) ? Math.max(1, Math.min(duration, expiresAt - Date.now())) : duration;
  });
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  useEffect(() => {
    if (expiresAt === undefined || !Number.isFinite(expiresAt)) return;
    let timer: number | undefined;
    const deadline = expiresAt;
    function refreshExpiry() {
      const remaining = deadline - Date.now();
      if (remaining <= 0) setExpired(true);
      else timer = window.setTimeout(refreshExpiry, Math.min(remaining, 2_147_483_647));
    }
    refreshExpiry();
    return () => window.clearTimeout(timer);
  }, [expiresAt]);
  function dismissOnce() {
    if (!active.current || dismissed.current) return;
    dismissed.current = true; dismiss.current();
  }
  async function runAction() {
    if (!action || inFlight.current || expired) return;
    if (expiresAt !== undefined && expiresAt <= Date.now()) { setExpired(true); return; }
    inFlight.current = true; setRunning(true);
    try { await action.run(); dismissOnce(); }
    catch { if (active.current) { inFlight.current = false; setRunning(false); } }
  }
  return <NotificationView id={String(message.id)} message={message.text} tone={message.kind} timeout={timeout} busy={running} onDismiss={dismissOnce}
    action={action ? <Button size="small" variant="secondary" disabled={running || expired} onClick={() => void runAction()}>{running ? "Restoring…" : expired ? `${action.label} expired` : action.label}</Button> : undefined} />;
}

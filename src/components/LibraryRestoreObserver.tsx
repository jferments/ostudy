import { restoredLibraryMessage } from "./libraryRestoreResult";
import { useEffect, useState } from "react";
import { getNativeLibrarySession, observeLibraryRestore } from "../ipc/librarySession";
import { refreshLibraryView } from "../platform/libraryView";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { Button } from "../ui/primitives/Button";

const SEEN = "ostudy.last-restore-notice";
const ROLLBACK = "ostudy.restore-rollback-notice";

/** Native completion also reaches views whose initiating IPC reply was lost. */
export function LibraryRestoreObserver() {
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;
    try {
      const rollback = sessionStorage.getItem(ROLLBACK);
      if (rollback) {
        setMessage(rollback);
        sessionStorage.removeItem(ROLLBACK);
      }
    } catch { /* A storage restriction must not block the restored library. */ }
    void observeLibraryRestore(
      (result) => {
        if (!disposed && result.completed && !result.restartRequired) refreshLibraryView();
      },
      (detail) => {
        if (disposed) return;
        try { sessionStorage.setItem(ROLLBACK, detail); } catch { /* Native receipt remains authoritative. */ }
        refreshLibraryView();
      },
    ).then((unsubscribe) => {
      if (disposed) unsubscribe();
      else cleanup = unsubscribe;
    }).catch(() => { /* The initiating restore response remains another completion path. */ });
    void getNativeLibrarySession().then((session) => {
      const result = session?.lastRestore;
      if (disposed || !result?.completed) return;
      try {
        if (localStorage.getItem(SEEN) === result.operationId) return;
        localStorage.setItem(SEEN, result.operationId);
      } catch { /* Showing a repeated notice is safer than hiding completion. */ }
      setMessage(restoredLibraryMessage(result));
    }).catch(() => { /* Startup exposes recovery failures separately. */ });
    return () => { disposed = true; cleanup?.(); };
  }, []);
  if (!message) return null;
  return <InlineNotice kind="info">
    <span role="status">{message}</span>
    <Button type="button" variant="secondary" onClick={() => setMessage(null)}>Dismiss</Button>
  </InlineNotice>;
}

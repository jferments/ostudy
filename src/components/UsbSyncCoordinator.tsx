import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../ipc/common";
import {
  getPendingUsbSyncRequest,
  joinUsbDevicePairing,
  listenToDeviceSyncProgress,
  rejectUsbDevicePairing,
  type DeviceSyncProgress,
  type PendingUsbSyncRequest,
} from "../ipc/deviceSync";
import { listenToAndroidUsbSync } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { Button } from "../ui/primitives/Button";
import { Progress } from "../ui/primitives/Progress";
import { Dialog, DialogActions } from "../ui/overlays/Dialog";
import type { ShowToast } from "./Toast";

interface UsbSyncCoordinatorProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

const USB_DISCOVERY_FAST_POLL_INTERVAL_MS = 750;
const USB_DISCOVERY_IDLE_POLL_INTERVAL_MS = 5_000;
const USB_DISCOVERY_FAST_WINDOW_MS = 12_000;

type UsbConsentPhase = "pending" | "accepting" | "rejecting" | "receiving";

/**
 * Discovers Android Open Accessory requests without claiming them. The native
 * descriptor remains pending until the learner explicitly accepts or rejects
 * the request. Only then does Rust take ownership and continue the shared,
 * authenticated device-sync protocol.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const android = capabilities.native?.operatingSystem === "android";
  const [pending, setPending] = useState<PendingUsbSyncRequest | null>(null);
  const [phase, setPhase] = useState<UsbConsentPhase>("pending");
  const [progress, setProgress] = useState<DeviceSyncProgress | null>(null);
  const receiving = useRef(false);

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let pollTimer: number | undefined;
    let fastUntil = Date.now() + USB_DISCOVERY_FAST_WINDOW_MS;

    function pollDelay(): number {
      return document.visibilityState === "visible" && Date.now() < fastUntil
        ? USB_DISCOVERY_FAST_POLL_INTERVAL_MS
        : USB_DISCOVERY_IDLE_POLL_INTERVAL_MS;
    }

    function schedulePoll(delay = pollDelay()): void {
      if (disposed) return;
      if (pollTimer !== undefined) window.clearTimeout(pollTimer);
      pollTimer = window.setTimeout(() => {
        pollTimer = undefined;
        void discoverPending().finally(() => schedulePoll());
      }, delay);
    }

    function armFastPolling(): void {
      fastUntil = Date.now() + USB_DISCOVERY_FAST_WINDOW_MS;
      schedulePoll(0);
    }

    async function discoverPending(): Promise<void> {
      if (disposed || receiving.current) return;
      try {
        const request = await getPendingUsbSyncRequest();
        if (disposed || !request.available) return;
        setPending(request);
        setPhase("pending");
      } catch {
        // Discovery is a best-effort Android-only convenience. An attachment
        // event can still surface the same pending descriptor.
      }
    }

    const handleFocus = (): void => armFastPolling();
    const handleVisibility = (): void => {
      if (document.visibilityState === "visible") armFastPolling();
    };
    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibility);

    void discoverPending().finally(() => schedulePoll());
    void listenToAndroidUsbSync((attachment) => {
      setPending({ available: true, ...attachment });
      setPhase("pending");
      armFastPolling();
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      if (pollTimer !== undefined) window.clearTimeout(pollTimer);
      window.removeEventListener("focus", handleFocus);
      document.removeEventListener("visibilitychange", handleVisibility);
      unlisten?.();
    };
  }, [android]);

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenToDeviceSyncProgress((next) => {
      if (!disposed && receiving.current && next.direction === "receive") setProgress(next);
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [android]);

  async function acceptRequest(): Promise<void> {
    if (receiving.current) return;
    receiving.current = true;
    setPhase("accepting");
    setProgress(null);
    try {
      setPhase("receiving");
      const result = await joinUsbDevicePairing();
      if (!result) throw new Error("The attached USB sync request was no longer available.");
      await refreshDecks();
      const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
      showToast(
        result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
        `USB sync received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
          result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
        }.${credentialWarning}`,
      );
      setPending(null);
      setProgress(null);
      setPhase("pending");
    } catch (error: unknown) {
      setPending(null);
      setProgress(null);
      setPhase("pending");
      showToast("error", getErrorMessage(error));
    } finally {
      receiving.current = false;
    }
  }

  async function rejectRequest(): Promise<void> {
    if (receiving.current) return;
    receiving.current = true;
    setPhase("rejecting");
    try {
      const rejected = await rejectUsbDevicePairing();
      if (!rejected) throw new Error("The attached USB sync request was no longer available.");
      showToast("info", "USB sync request rejected.");
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      receiving.current = false;
      setPending(null);
      setProgress(null);
      setPhase("pending");
    }
  }

  if (!android || !pending) return null;

  const transferring = phase === "receiving";
  const progressText = progress
    ? `Receiving ${formatBytes(progress.transferredBytes)} of ${formatBytes(progress.totalBytes)}`
    : "Preparing the encrypted USB transfer…";

  return (
    <Dialog
      title="USB sync request"
      description="Would you like to accept a USB sync request? This will let the sending device transfer selected flashcards, documents, study guides, and API provider information to this device. This device never sends its own library back."
      role="alertdialog"
      dismissible={false}
      busy={phase !== "pending"}
      onClose={() => undefined}
      width="small"
    >
      <div className="tw:grid tw:gap-4 tw:p-4">
        {transferring || phase === "accepting" ? (
          <Progress
            value={progress?.transferredBytes ?? null}
            max={Math.max(1, progress?.totalBytes ?? 1)}
            label="USB synchronization progress"
            valueText={progressText}
            text={<span>{progressText}</span>}
          />
        ) : phase === "rejecting" ? (
          <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">Rejecting the request…</p>
        ) : (
          <p className="tw:m-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground">
            Nothing will be transferred unless you accept this request.
          </p>
        )}
        <DialogActions>
          <Button
            variant="secondary"
            disabled={phase !== "pending"}
            onClick={() => void rejectRequest()}
          >
            Reject
          </Button>
          <Button
            variant="primary"
            disabled={phase !== "pending"}
            onClick={() => void acceptRequest()}
          >
            Accept
          </Button>
        </DialogActions>
      </div>
    </Dialog>
  );
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let amount = value;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount >= 10 || unit === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[unit]}`;
}

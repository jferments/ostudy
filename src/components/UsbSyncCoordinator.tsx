import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../ipc/common";
import {
  getPendingUsbSyncRequest,
  joinUsbDevicePairing,
  listenToDeviceSyncProgress,
  requestUsbAccessoryPermission,
  type DeviceSyncProgress,
  type PendingUsbSyncRequest,
} from "../ipc/deviceSync";
import { listenToAndroidUsbSync } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { Panel } from "../ui/primitives/Panel";
import { Progress } from "../ui/primitives/Progress";
import type { ShowToast } from "./Toast";

interface UsbSyncCoordinatorProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

const USB_DISCOVERY_FAST_POLL_INTERVAL_MS = 750;
const USB_DISCOVERY_IDLE_POLL_INTERVAL_MS = 5_000;
const USB_DISCOVERY_FAST_WINDOW_MS = 12_000;

type UsbReceivePhase = "pending" | "authorizing" | "receiving";

/**
 * Android owns the one user-consent prompt for an AOA attachment. Once Android
 * routes the accessory to oStudy, this coordinator claims the granted
 * accessory and starts receiving automatically. There is intentionally no
 * second app-owned Accept/Reject prompt.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const android = capabilities.native?.operatingSystem === "android";
  const [pending, setPending] = useState<PendingUsbSyncRequest | null>(null);
  const [phase, setPhase] = useState<UsbReceivePhase>("pending");
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
        // Discovery is best-effort. Android owns the attachment/permission UI;
        // the next bounded poll retries without creating another consent layer.
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
      if (receiving.current) return;
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

  useEffect(() => {
    if (!pending || phase !== "pending" || receiving.current) return;
    void receiveRequest();
  }, [pending, phase]);

  async function receiveRequest(): Promise<void> {
    if (receiving.current) return;
    receiving.current = true;
    setPhase("authorizing");
    setProgress(null);
    try {
      // Android grants temporary accessory access when the user allows oStudy
      // to handle the USB_ACCESSORY_ATTACHED intent. This call verifies that
      // OS-owned permission and opens the accessory; it is not an app consent.
      const permission = await requestUsbAccessoryPermission();
      if (!permission.available) {
        throw new Error("The attached USB sync request was no longer available.");
      }
      if (!permission.granted) {
        setPending(null);
        setProgress(null);
        setPhase("pending");
        showToast("info", "Android did not grant USB access to oStudy.");
        return;
      }

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

  if (!android || !pending || (phase !== "authorizing" && phase !== "receiving")) return null;

  const progressText = progress
    ? `Receiving ${formatBytes(progress.transferredBytes)} of ${formatBytes(progress.totalBytes)}`
    : "Opening the Android USB connection…";

  return (
    <Panel
      as="aside"
      aria-live="polite"
      title="Receiving USB sync"
      className="tw:fixed tw:right-4 tw:bottom-[calc(var(--safe-bottom)+var(--android-ime-bottom)+5.5rem)] tw:z-[650] tw:w-[calc(100vw-2rem)] tw:max-w-sm tw:shadow-overlay"
    >
      <Progress
        value={progress?.transferredBytes ?? null}
        max={Math.max(1, progress?.totalBytes ?? 1)}
        label="USB synchronization progress"
        valueText={progressText}
        text={<span>{progressText}</span>}
      />
    </Panel>
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

import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../ipc/common";
import {
  getPendingUsbSyncRequest,
  joinUsbDevicePairing,
  listenToDeviceSyncProgress,
  rejectUsbDevicePairing,
  requestUsbAccessoryPermission,
  type DeviceSyncProgress,
  type PendingUsbSyncRequest,
} from "../ipc/deviceSync";
import { listenToAndroidUsbSync } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { useConfirm } from "../ui/overlays/overlayContext";
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

type UsbConsentPhase = "pending" | "accepting" | "rejecting" | "receiving";

/**
 * Discovers Android Open Accessory requests without opening them or requesting
 * OS permission. The learner sees oStudy's Accept/Reject explanation first. An
 * Accept may then request Android USB access; only after that grant does Rust
 * take descriptor ownership and continue the authenticated sync protocol.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const confirm = useConfirm();
  const android = capabilities.native?.operatingSystem === "android";
  const [pending, setPending] = useState<PendingUsbSyncRequest | null>(null);
  const [phase, setPhase] = useState<UsbConsentPhase>("pending");
  const [progress, setProgress] = useState<DeviceSyncProgress | null>(null);
  const receiving = useRef(false);
  const consentPromptOpen = useRef(false);

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
        // Discovery is best-effort. The next bounded poll retries without
        // opening the accessory or triggering Android's permission UI.
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
      // The native plugin emits this only after Android USB permission has
      // already been granted. Consent/transfer is already in progress then, so
      // never open a second oStudy prompt for the same request.
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
    if (!pending || phase !== "pending" || consentPromptOpen.current) return;
    consentPromptOpen.current = true;
    void confirm({
      title: "USB sync request",
      description:
        "Accept this USB sync request? Android may show a system USB access prompt next; accept it to continue. The sending device can transfer selected flashcards, documents, study guides, and API provider information to this device. This device never sends its own library back.",
      confirmLabel: "Accept",
      cancelLabel: "Reject",
    }).then((accepted) => {
      consentPromptOpen.current = false;
      if (accepted) void acceptRequest();
      else void rejectRequest();
    });
  }, [confirm, pending, phase]);

  async function acceptRequest(): Promise<void> {
    if (receiving.current) return;
    receiving.current = true;
    setPhase("accepting");
    setProgress(null);
    try {
      const permission = await requestUsbAccessoryPermission();
      if (!permission.available) {
        throw new Error("The attached USB sync request was no longer available.");
      }
      if (!permission.granted) {
        setPending(null);
        setProgress(null);
        setPhase("pending");
        showToast("info", "USB sync was not authorized in Android.");
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

  if (!android || !pending || (phase !== "accepting" && phase !== "receiving")) return null;

  const progressText = progress
    ? `Receiving ${formatBytes(progress.transferredBytes)} of ${formatBytes(progress.totalBytes)}`
    : "Preparing the encrypted USB transfer…";

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

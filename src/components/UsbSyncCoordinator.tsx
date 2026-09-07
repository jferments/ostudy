import { useEffect, useRef } from "react";

import { getErrorMessage } from "../ipc/common";
import { joinUsbDevicePairing } from "../ipc/deviceSync";
import { listenToAndroidUsbSync } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import type { ShowToast } from "./Toast";

interface UsbSyncCoordinatorProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

const USB_CLAIM_FAST_POLL_INTERVAL_MS = 750;
const USB_CLAIM_IDLE_POLL_INTERVAL_MS = 5_000;
const USB_CLAIM_FAST_WINDOW_MS = 12_000;

/**
 * Claims Android Open Accessory connections as soon as the native platform
 * adapter reports them. The cable transports the same Rust-owned authenticated
 * synchronization protocol as LAN; there is no QR, IP address, tethering, ADB,
 * or WebView-owned transfer protocol on the USB path.
 *
 * Android normally brings the activity to the foreground for an accessory
 * attachment. The native listener is therefore only a fast path; an adaptive
 * foreground poll covers plugin-listener ACL differences without waking the
 * WebView twice per second indefinitely. While a transfer is active the claim
 * call itself remains pending, so no additional polls are scheduled.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const receiving = useRef(false);
  const android = capabilities.native?.operatingSystem === "android";

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let pollTimer: number | undefined;
    let fastUntil = Date.now() + USB_CLAIM_FAST_WINDOW_MS;
    let reportedReceiveError = false;

    function pollDelay(): number {
      return document.visibilityState === "visible" && Date.now() < fastUntil
        ? USB_CLAIM_FAST_POLL_INTERVAL_MS
        : USB_CLAIM_IDLE_POLL_INTERVAL_MS;
    }

    function schedulePoll(delay = pollDelay()): void {
      if (disposed) return;
      if (pollTimer !== undefined) window.clearTimeout(pollTimer);
      pollTimer = window.setTimeout(() => {
        pollTimer = undefined;
        void receivePending().finally(() => schedulePoll());
      }, delay);
    }

    function armFastPolling(): void {
      fastUntil = Date.now() + USB_CLAIM_FAST_WINDOW_MS;
      schedulePoll(0);
    }

    async function receivePending(): Promise<void> {
      if (disposed || receiving.current) return;
      receiving.current = true;
      try {
        const result = await joinUsbDevicePairing();
        if (!result || disposed) return;
        reportedReceiveError = false;
        await refreshDecks();
        const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
        showToast(
          result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
          `USB sync received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
            result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
          }.${credentialWarning}`,
        );
      } catch (error: unknown) {
        if (!disposed && !reportedReceiveError) {
          reportedReceiveError = true;
          showToast("error", getErrorMessage(error));
        }
      } finally {
        receiving.current = false;
      }
    }

    const handleFocus = (): void => armFastPolling();
    const handleVisibility = (): void => {
      if (document.visibilityState === "visible") armFastPolling();
    };
    window.addEventListener("focus", handleFocus);
    document.addEventListener("visibilitychange", handleVisibility);

    // Claim a descriptor that may have arrived while the WebView was starting.
    void receivePending().finally(() => schedulePoll());

    // Keep the native event fast path when the runtime exposes its listener ACL.
    // The adaptive poll above remains sufficient when that namespace is denied.
    void listenToAndroidUsbSync(() => {
      armFastPolling();
      void receivePending();
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
  }, [android, refreshDecks, showToast]);

  return null;
}

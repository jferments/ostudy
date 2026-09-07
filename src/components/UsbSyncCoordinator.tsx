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

const USB_CLAIM_POLL_INTERVAL_MS = 500;

/**
 * Claims Android Open Accessory connections as soon as the native platform
 * adapter reports them. The cable transports the same Rust-owned authenticated
 * synchronization protocol as LAN; there is no QR, IP address, tethering, ADB,
 * or WebView-owned transfer protocol on the USB path.
 *
 * The native event subscription is best-effort because the dedicated Android
 * USB plugin shares one Rust crate/ACL namespace with the broader platform
 * plugin. A short foreground poll is therefore the compatibility path that
 * makes descriptor claiming independent of WebView plugin-listener ACLs.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const receiving = useRef(false);
  const android = capabilities.native?.operatingSystem === "android";

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let reportedReceiveError = false;

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

    // Claim a descriptor that may have arrived while the WebView was starting,
    // then keep a bounded-rate compatibility poll alive for accessory intents
    // that arrive later. A null result is the ordinary "nothing attached" path.
    void receivePending();
    const poll = window.setInterval(() => void receivePending(), USB_CLAIM_POLL_INTERVAL_MS);

    // Keep the native event fast path when the runtime exposes its listener ACL.
    // Current Android builds may reject this separate plugin namespace; the poll
    // above is intentionally sufficient on its own, so that rejection is quiet.
    void listenToAndroidUsbSync(() => void receivePending())
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      window.clearInterval(poll);
      unlisten?.();
    };
  }, [android, refreshDecks, showToast]);

  return null;
}

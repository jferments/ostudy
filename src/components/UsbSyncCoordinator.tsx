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

/**
 * Claims Android Open Accessory connections as soon as the native platform
 * adapter reports them. The cable transports the same Rust-owned authenticated
 * synchronization protocol as LAN; there is no QR, IP address, tethering, ADB,
 * or WebView-owned transfer protocol on the USB path.
 */
export function UsbSyncCoordinator({ refreshDecks, showToast }: UsbSyncCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const receiving = useRef(false);
  const android = capabilities.native?.operatingSystem === "android";

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    async function receivePending(): Promise<void> {
      if (disposed || receiving.current) return;
      receiving.current = true;
      try {
        const result = await joinUsbDevicePairing();
        if (!result || disposed) return;
        await refreshDecks();
        const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
        showToast(
          result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
          `USB sync received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
            result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
          }.${credentialWarning}`,
        );
      } catch (error: unknown) {
        if (!disposed) showToast("error", getErrorMessage(error));
      } finally {
        receiving.current = false;
      }
    }

    // Claim a descriptor that may have arrived while the WebView was starting.
    void receivePending();
    void listenToAndroidUsbSync(() => void receivePending())
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Unable to register the Android USB-sync listener:", getErrorMessage(error));
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [android, refreshDecks, showToast]);

  return null;
}

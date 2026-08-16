import { useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { scanDeviceSyncQr } from "../../ipc/deviceSyncQr";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";

interface AndroidSyncQrScannerProps {
  disabled: boolean;
  onScanned: (invitation: string) => void;
  onError: (message: string) => void;
}

export function AndroidSyncQrScanner({
  disabled,
  onScanned,
  onError,
}: AndroidSyncQrScannerProps) {
  const [scanning, setScanning] = useState(false);
  const platform = usePlatformCapabilities();
  const isAndroid = platform.native?.operatingSystem === "android";
  if (!isAndroid) return null;

  async function scan(): Promise<void> {
    if (scanning || disabled) return;
    setScanning(true);
    try {
      const invitation = await scanDeviceSyncQr();
      if (invitation) onScanned(invitation);
    } catch (error: unknown) {
      onError(getErrorMessage(error));
    } finally {
      setScanning(false);
    }
  }

  return (
    <button
      type="button"
      className="secondary-button"
      disabled={disabled || scanning}
      onClick={() => void scan()}
    >
      {scanning ? "Opening camera…" : "Scan QR with camera"}
    </button>
  );
}

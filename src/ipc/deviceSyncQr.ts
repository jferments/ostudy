import { invoke } from "@tauri-apps/api/core";

export async function scanDeviceSyncQr(): Promise<string | null> {
  return invoke<string | null>("scan_device_pairing_qr");
}

#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [transport, usbHost, usbPlugin, usbService, mobileAdapter, manifest, coordinator, deviceSyncIpc] = await Promise.all([
  read("src-tauri/src/device_sync/transport.rs"),
  read("src-tauri/src/device_sync/usb.rs"),
  read("plugins/ostudy-platform/android/src/main/java/OstudyUsbPlugin.kt"),
  read("plugins/ostudy-platform/android/src/main/java/OstudyUsbSyncService.kt"),
  read("plugins/ostudy-platform/src/mobile.rs"),
  read("plugins/ostudy-platform/android/src/main/AndroidManifest.xml"),
  read("src/components/UsbSyncCoordinator.tsx"),
  read("src/ipc/deviceSync.ts"),
]);

const failures = [];

expectContains(transport, "device_sync_completion_unconfirmed", "sender distinguishes a lost post-transfer completion receipt");
expectContains(transport, "device_sync_completion_acknowledgement_lost", "receiver records a lost post-commit completion acknowledgement without rolling back success");
expectContains(transport, "committed_receiver_does_not_fail_when_final_receipt_cannot_be_written", "post-commit acknowledgement regression test");
expectContains(transport, "missing_final_receipt_has_completion_unconfirmed_semantics", "sender completion-unconfirmed regression test");
expectContains(transport, "send_file_with_progress", "encrypted transfer reports sender byte progress");
expectContains(transport, "receive_file_with_progress", "encrypted transfer reports receiver byte progress");

expectNotContains(usbHost, "fn libusb_set_configuration(", "Linux AOA host does not declare whole-device configuration reset");
expectNotContains(usbHost, "libusb_set_configuration(self.handle", "Linux AOA host does not reset the whole composite USB configuration");
expectContains(usbHost, "claim the Android accessory interface", "Linux AOA host claims only the accessory interface");
expectContains(usbHost, '"oStudy USB sync request"', "Android system accessory chooser receives a concise human-readable description");

expectContains(usbPlugin, "OstudyUsbSyncService.begin(activity, current.fd)", "Android starts the USB transfer lifetime guard when Rust claims the descriptor");
expectContains(usbPlugin, "EMPTY_CLAIM_LOG_INTERVAL_MILLIS", "empty USB claim diagnostics are rate limited");
expectContains(usbPlugin, "fun peekUsbAccessory", "Android can inspect a pending accessory without consuming it");
expectContains(usbPlugin, "fun requestUsbAccessoryPermission", "Android keeps system permission mechanics inside the private native adapter");
expectContains(usbPlugin, "suppressedKey = accessoryKey(accessory)", "Android denial suppresses repeated permission prompts until detach/reconnect");
expectNotContains(usbPlugin, "intent?.let(::accessoryFromIntent)", "Kotlin accessory intent processing avoids the redundant nullable safe-call warning");
expectNotContains(usbPlugin, "accessory.manufacturer ?: OSTUDY_ACCESSORY_MANUFACTURER", "Kotlin accessory manufacturer does not use a redundant Elvis fallback");
expectNotContains(usbPlugin, "accessory.model ?: OSTUDY_ACCESSORY_MODEL", "Kotlin accessory model does not use a redundant Elvis fallback");

expectContains(mobileAdapter, 'run_mobile_plugin("requestUsbAccessoryPermission", ())', "Rust privately requests Android USB authorization before claiming the receiver");
expectContains(mobileAdapter, 'run_mobile_plugin("takeUsbAccessory", ())', "Rust privately claims the prepared Android accessory descriptor");

expectContains(usbService, "ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC", "USB transfer uses Android dataSync foreground-service type");
expectContains(usbService, "PowerManager.PARTIAL_WAKE_LOCK", "USB transfer holds a bounded partial wake lock");
expectContains(usbService, "ParcelFileDescriptor.fromFd(fd)", "foreground service follows the actual Rust-owned descriptor lifetime");
expectContains(usbService, "override fun onTimeout", "foreground service handles Android dataSync timeout");
expectContains(manifest, 'android:name=".OstudyUsbSyncService"', "USB foreground service is registered");
expectContains(manifest, 'android:foregroundServiceType="dataSync"', "Android manifest declares dataSync foreground-service capability");

expectContains(coordinator, "USB_DISCOVERY_FAST_POLL_INTERVAL_MS", "Android USB coordinator has a bounded fast discovery window");
expectContains(coordinator, "USB_DISCOVERY_IDLE_POLL_INTERVAL_MS", "Android USB coordinator backs off while idle");
expectContains(coordinator, "getPendingUsbSyncRequest()", "Android USB discovery remains passive before connection establishment");
expectContains(coordinator, "void acceptRequest()", "Android USB coordinator explicitly starts the shared connection path");
expectContains(coordinator, "joinUsbDevicePairing()", "Android USB coordinator enters USB through the shared join command");
expectNotContains(coordinator, "requestUsbAccessoryPermission", "frontend never invokes Android USB permission mechanics directly");
expectNotContains(deviceSyncIpc, "plugin:ostudy-usb|", "shared frontend IPC never bypasses Rust to invoke the private Android USB plugin");
expectContains(coordinator, 'document.addEventListener("visibilitychange"', "Android USB coordinator re-arms around foreground activity");
expectContains(coordinator, "window.setTimeout", "Android USB coordinator schedules non-overlapping adaptive polls");
expectNotContains(coordinator, "window.setInterval(() => void receivePending()", "continuous 500 ms USB polling is removed");

if (failures.length > 0) {
  console.error("USB synchronization hardening verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log("OK   USB sync preserves post-commit truth, keeps Android authorization behind Rust, reports byte progress, keeps Android transfers alive, claims only the AOA interface, and backs off idle discovery.");

async function read(relativePath) {
  return readFile(new URL(relativePath, `file://${root}/`), "utf8");
}

function expectContains(content, needle, label) {
  if (!content.includes(needle)) failures.push(`Missing ${label}: ${JSON.stringify(needle)}`);
}

function expectNotContains(content, needle, label) {
  if (content.includes(needle)) failures.push(`Unexpected ${label}: ${JSON.stringify(needle)}`);
}

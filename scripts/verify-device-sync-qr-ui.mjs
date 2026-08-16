#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [nativeScanner, qrCommand, frontendIpc, compositionRoot, settingsUi, settingsCss, syncCore, syncPackage] =
  await Promise.all([
    read("plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"),
    read("src-tauri/src/device_sync_qr_commands.rs"),
    read("src/ipc/deviceSync.ts"),
    read("src-tauri/src/lib.rs"),
    read("src/screens/settings/DeviceSyncSettings.tsx"),
    read("src/screens/settings/DeviceSyncSettings.module.css"),
    read("src-tauri/src/device_sync/mod.rs"),
    read("src-tauri/src/device_sync/package.rs"),
  ]);
const failures = [];

expectContains(nativeScanner, "BarcodeFormat.QR_CODE.name", "QR-only native scan format");
expectContains(nativeScanner, ".setBeepEnabled(true)", "native detection feedback");
expectContains(nativeScanner, ".setCameraId(0)", "rear-camera selection");
expectContains(nativeScanner, "QR_SCAN_TIMEOUT_MILLIS", "bounded native scan lifetime");
expectContains(nativeScanner, "Center the entire ostudy pairing code inside the frame", "native scan guidance");
expectContains(qrCommand, "pub async fn scan_device_pairing_qr_async", "async QR command adapter");
expectContains(qrCommand, "tauri::async_runtime::spawn_blocking", "QR mobile-plugin worker handoff");
expectContains(frontendIpc, 'invoke<string | null>("scan_device_pairing_qr_async")', "frontend async QR invocation");
expectContains(frontendIpc, "includeApiSettings", "sender-controlled API settings scope");
expectNotContains(frontendIpc, "includeCredentials", "removed receiver credential consent field");
expectContains(
  compositionRoot,
  "device_sync_qr_commands::scan_device_pairing_qr_async",
  "async QR command registration",
);
expectContains(settingsUi, 'role="status"', "accessible scanner result status");
expectContains(settingsUi, "Sync offer detected", "visual offer detection result");
expectContains(settingsUi, "await receiveOffer(scanned, true)", "automatic receive after QR detection");
expectContains(settingsUi, "Accept sync offer", "text-invitation acceptance action");
expectContains(settingsUi, "Include API provider settings and saved credentials", "sender API scope control");
expectContains(settingsUi, "app settings always stay local", "device-local app-settings explanation");
expectContains(settingsUi, "entire white border visible", "host-side QR framing guidance");
expectContains(settingsCss, "width: min(100%, 340px);", "large desktop pairing QR presentation");
expectContains(settingsCss, "background: #ffffff;", "high-contrast QR quiet-zone background");
expectContains(syncCore, "const PROTOCOL_VERSION: u32 = 2;", "one-way offer protocol version");
expectContains(syncCore, "sent_package_bytes: 0", "receiver does not upload a package during offer acceptance");
expectContains(syncPackage, "DELETE FROM app_settings;", "app settings removed before package transmission");
expectContains(syncPackage, "DELETE FROM ai_task_defaults;", "API settings excluded when sender leaves scope off");
expectContains(syncPackage, "DELETE FROM ai_provider_profiles;", "API profiles excluded when sender leaves scope off");

if (failures.length > 0) {
  console.error("Device-sync QR/offer contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Device-sync QR uses nonblocking native delivery; one scan accepts a sender-scoped offer, local app settings stay off the wire, and API settings are sender opt-in.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectNotContains(source, forbidden, label) {
  if (source.includes(forbidden)) failures.push(`Forbidden ${label}: ${forbidden}`);
}

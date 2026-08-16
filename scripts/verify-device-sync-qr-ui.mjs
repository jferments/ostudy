#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [nativeScanner, settingsUi, settingsCss] = await Promise.all([
  read("plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"),
  read("src/screens/settings/DeviceSyncSettings.tsx"),
  read("src/screens/settings/DeviceSyncSettings.module.css"),
]);
const failures = [];

expectContains(nativeScanner, "BarcodeFormat.QR_CODE.name", "QR-only native scan format");
expectContains(nativeScanner, ".setBeepEnabled(true)", "native detection feedback");
expectContains(nativeScanner, ".setCameraId(0)", "rear-camera selection");
expectContains(nativeScanner, "QR_SCAN_TIMEOUT_MILLIS", "bounded native scan lifetime");
expectContains(nativeScanner, "Center the entire ostudy pairing code inside the frame", "native scan guidance");
expectContains(settingsUi, 'role="status"', "accessible scanner result status");
expectContains(settingsUi, "Pairing code detected", "visual detection result");
expectContains(settingsUi, "entire white border visible", "host-side QR framing guidance");
expectContains(settingsCss, "width: min(100%, 340px);", "large desktop pairing QR presentation");
expectContains(settingsCss, "background: #ffffff;", "high-contrast QR quiet-zone background");

if (failures.length > 0) {
  console.error("Device-sync QR usability contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Device-sync QR UI keeps a large high-contrast code, native scan guidance, and explicit detection feedback.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

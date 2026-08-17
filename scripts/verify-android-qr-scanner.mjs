#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)), "..");
const paths = {
  gradle: resolve(root, "plugins/ostudy-platform/android/build.gradle.kts"),
  manifest: resolve(root, "plugins/ostudy-platform/android/src/main/AndroidManifest.xml"),
  implementation: resolve(
    root,
    "plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt",
  ),
};
const [gradle, manifest, implementation] = await Promise.all(
  Object.values(paths).map((path) => readFile(path, "utf8")),
);

function requireText(source, text, message) {
  if (!source.includes(text)) throw new Error(message);
}

function forbidText(source, text, message) {
  if (source.includes(text)) throw new Error(message);
}

requireText(
  gradle,
  'implementation("com.google.android.gms:play-services-code-scanner:16.1.0")',
  "The audited Google Code Scanner dependency is missing or changed.",
);
forbidText(gradle, "journeyapps", "ZXing Embedded must not remain in the Android dependency graph.");
forbidText(gradle, "zxing-android", "ZXing Android dependencies must not remain in the plugin.");
requireText(
  manifest,
  'android:name="com.google.mlkit.vision.DEPENDENCIES"',
  "The install-time ML Kit dependency metadata is missing.",
);
requireText(
  manifest,
  'android:value="barcode_ui"',
  "The Google Code Scanner barcode_ui module is not requested at install time.",
);
forbidText(
  manifest,
  "android.permission.CAMERA",
  "ostudy must not request camera permission when Google Code Scanner owns the camera UI.",
);
requireText(
  implementation,
  ".setBarcodeFormats(Barcode.FORMAT_QR_CODE)",
  "The scanner must restrict recognition to QR codes.",
);
requireText(
  implementation,
  ".enableAutoZoom()",
  "The scanner should enable first-party auto zoom for accessibility and reliability.",
);
requireText(
  implementation,
  "GmsBarcodeScanning.getClient(activity, options)",
  "The QR plugin is not using Google Code Scanner.",
);
requireText(
  implementation,
  ".startScan()",
  "The QR plugin does not start the first-party scanner task.",
);
requireText(
  implementation,
  "AtomicBoolean",
  "The scanner must guard against concurrent and duplicate completion callbacks.",
);
forbidText(
  implementation,
  "IntentIntegrator",
  "The maintenance-mode ZXing IntentIntegrator implementation must be removed.",
);
forbidText(
  implementation,
  "contents}",
  "Raw QR payloads must never be interpolated into Android diagnostic messages.",
);

console.log("OK   Android QR scanning uses permissionless Google Code Scanner with bounded results.");

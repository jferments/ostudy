#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const paths = {
  gradle: resolve(root, "plugins/ostudy-platform/android/build.gradle.kts"),
  manifest: resolve(root, "plugins/ostudy-platform/android/src/main/AndroidManifest.xml"),
  qr: resolve(root, "plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"),
  document: resolve(
    root,
    "plugins/ostudy-platform/android/src/main/java/OstudyDocumentPlugin.kt",
  ),
};
const [gradle, manifest, qr, document] = await Promise.all(
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
  'implementation("com.journeyapps:zxing-android-embedded:4.3.0")',
  "The audited Apache-2.0 ZXing Embedded dependency is missing or changed.",
);
requireText(
  gradle,
  'implementation("com.google.zxing:core:3.5.4")',
  "The audited Apache-2.0 ZXing core dependency is missing or changed.",
);
requireText(
  gradle,
  'implementation("com.github.adaptech-cz.Tesseract4Android:tesseract4android:4.9.0")',
  "The audited Apache-2.0 Tesseract4Android dependency is missing or changed.",
);
requireText(
  gradle,
  'val tesseractDataCommit = "87416418657359cb625c412a48b6e1d6d41c29bd"',
  "The bundled Tesseract model source is not pinned to the audited upstream commit.",
);
requireText(
  gradle,
  'val tesseractEnglishBlobSha1 = "bbef4675053b5b468cdb477053e28b1c698ba08e"',
  "The bundled Tesseract model does not verify its immutable upstream Git blob.",
);
for (const proprietary of [
  "com.google.mlkit:text-recognition",
  "com.google.android.gms:play-services-code-scanner",
  "GmsBarcodeScanning",
  "TextRecognition.getClient",
]) {
  forbidText(
    `${gradle}\n${qr}\n${document}`,
    proprietary,
    `Proprietary Android imaging dependency/runtime must not remain: ${proprietary}`,
  );
}
requireText(
  manifest,
  'android.permission.CAMERA',
  "The local QR scanner must declare camera permission explicitly.",
);
forbidText(
  manifest,
  "com.google.mlkit.vision.DEPENDENCIES",
  "Google scanner module metadata must not remain in the Android manifest.",
);
requireText(
  qr,
  "ScanContract()",
  "The QR plugin is not using the local ZXing Activity Result contract.",
);
requireText(
  qr,
  ".setDesiredBarcodeFormats(ScanOptions.QR_CODE)",
  "The local scanner must restrict recognition to QR codes.",
);
requireText(
  qr,
  ".setBarcodeImageEnabled(false)",
  "The scanner must not retain camera-frame image files.",
);
requireText(
  qr,
  "AtomicBoolean",
  "The scanner must guard against concurrent and duplicate completion callbacks.",
);
forbidText(
  qr,
  "contents}",
  "Raw QR payloads must never be interpolated into Android diagnostic messages.",
);
requireText(
  document,
  "TessBaseAPI()",
  "Android OCR is not using the local Tesseract engine.",
);
requireText(
  document,
  'activity.assets.open(TESSDATA_ASSET)',
  "Android OCR must install its packaged trained data without a runtime download.",
);
requireText(
  document,
  "future.get(timeoutSeconds.toLong(), TimeUnit.SECONDS)",
  "Android OCR must retain a bounded native recognition timeout.",
);

console.log(
  "OK   Android QR and OCR use bounded local open-source ZXing/Tesseract runtimes with packaged model data.",
);

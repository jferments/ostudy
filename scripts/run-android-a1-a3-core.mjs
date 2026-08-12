#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const applicator = join(root, "scripts", "apply-android-a1-a3-core.mjs");
let source = await readFile(applicator, "utf8");

const wrongPluginPath =
  "plugins/ostudy-platform/android/src/main/java/io/github/jferments/ostudy/platform/OstudyPlatformPlugin.kt";
const correctPluginPath = "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt";
if (!source.includes(wrongPluginPath)) {
  throw new Error("The Android plugin source-path normalization anchor was not found.");
}
source = source.replaceAll(wrongPluginPath, correctPluginPath);

// Tauri chooses APK/AAB from the CLI flags. Keeping desktop-style bundle
// targets in the mobile overlay is rejected by some Tauri schema revisions.
source = source.replace('      targets: ["apk", "aab"],\n', "");

const testDependency =
  '    androidTestImplementation("androidx.test.ext:junit:1.2.1")\n';
const testDependencyReplacement =
  `${testDependency}    androidTestImplementation("androidx.test:core-ktx:1.6.1")\n`;
if (!source.includes(testDependency)) {
  throw new Error("The Android instrumentation dependency anchor was not found.");
}
source = source.replace(testDependency, testDependencyReplacement);

await writeFile(applicator, source, "utf8");
await import(new URL("./apply-android-a1-a3-core.mjs", import.meta.url));

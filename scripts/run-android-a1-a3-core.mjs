#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const applicator = join(root, "scripts", "apply-android-a1-a3-core.mjs");
let source = await readFile(applicator, "utf8");
const wrong = "plugins/ostudy-platform/android/src/main/java/io/github/jferments/ostudy/platform/OstudyPlatformPlugin.kt";
const correct = "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt";
if (!source.includes(wrong)) {
  throw new Error("The Android plugin source-path normalization anchor was not found.");
}
source = source.replaceAll(wrong, correct);
await writeFile(applicator, source, "utf8");
await import(new URL("./apply-android-a1-a3-core.mjs", import.meta.url));

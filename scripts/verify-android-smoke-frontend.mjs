#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const mode = process.argv[2] ?? "";
if (!new Set(["present", "absent"]).has(mode)) {
  throw new Error("Usage: verify-android-smoke-frontend.mjs <present|absent>");
}

const assets = resolve(root, "dist", "assets");
const names = (await readdir(assets)).filter((name) => /^androidTestBridge-.*\.js$/u.test(name)).sort();
const markerMatches = [];
for (const name of names) {
  const text = await readFile(resolve(assets, name), "utf8");
  if (text.includes("__OSTUDY_ANDROID_TEST__")) markerMatches.push(name);
}

if (mode === "present") {
  if (names.length !== 1 || markerMatches.length !== 1) {
    throw new Error(
      `Android smoke frontend must contain exactly one marked test bridge chunk; chunks=${JSON.stringify(names)} marked=${JSON.stringify(markerMatches)}`,
    );
  }
  console.log(`OK   Android smoke frontend includes ${names[0]}.`);
  process.exit(0);
}

if (names.length !== 0 || markerMatches.length !== 0) {
  throw new Error(
    `Normal Android frontend unexpectedly contains the emulator-only test bridge: ${JSON.stringify(names)}`,
  );
}
console.log("OK   Normal Android frontend excludes the emulator-only test bridge.");
